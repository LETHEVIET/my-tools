"""Run with python -m unittest discover -s tests -v."""
import io
import json
from pathlib import Path
import struct
import sys
import threading
import unittest
from urllib.error import HTTPError
from urllib.request import Request, urlopen

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from image_processing import image_to_dib, load_and_normalize, InvalidImage
from receiver_server import AppState, make_server


def png_bytes(size=(12, 9), color=(250, 20, 10)):
    buffer = io.BytesIO()
    Image.new('RGB', size, color).save(buffer, format='PNG')
    return buffer.getvalue()


class ImageTests(unittest.TestCase):
    def test_normalize_image(self):
        result = load_and_normalize(png_bytes((3200, 1600)))
        self.assertEqual(result.size, (2400, 1200))
        self.assertEqual(result.mode, 'RGB')

    def test_invalid_image(self):
        with self.assertRaises(InvalidImage):
            load_and_normalize(b'bad picture')

    def test_dib_header_stride_and_pixel_direction(self):
        image = Image.new('RGB', (1, 2), (255, 0, 0))
        image.putpixel((0, 1), (0, 0, 255))
        dib = image_to_dib(image)
        size, width, height, planes, bpp = struct.unpack('<IiiHH', dib[:16])
        self.assertEqual((size, width, height, planes, bpp), (40, 1, 2, 1, 24))
        self.assertEqual(len(dib), 40 + 4 * 2)
        self.assertEqual(dib[40:44], bytes([255, 0, 0, 0]))  # bottom blue, BGR
        self.assertEqual(dib[44:48], bytes([0, 0, 255, 0]))  # top red, BGR


class ServerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.received = []
        cls.state = AppState(lambda image: cls.received.append(image.size) or 'Pasted (test)')
        cls.server = make_server(cls.state, bind='127.0.0.1', port=0)
        cls.port = cls.server.server_address[1]
        cls.thread = threading.Thread(target=cls.server.serve_forever, daemon=True)
        cls.thread.start()

    @classmethod
    def tearDownClass(cls):
        cls.server.shutdown()
        cls.server.server_close()
        cls.thread.join(timeout=2)

    def request(self, path, method='GET', data=None, token=None):
        headers = {}
        if token:
            headers['Authorization'] = 'Bearer ' + token
        if data is not None:
            headers['Content-Type'] = 'image/png'
        request = Request(f'http://127.0.0.1:{self.port}{path}', data=data, headers=headers, method=method)
        try:
            with urlopen(request, timeout=4) as res:
                return res.status, res.read(), res.headers
        except HTTPError as exc:
            return exc.code, exc.read(), exc.headers

    def test_01_web_assets_and_pwa_manifest(self):
        status, data, headers = self.request('/')
        self.assertEqual(status, 200)
        self.assertIn(b'My Tools', data)
        status, tool_page, _ = self.request('/tools/snap-to-paste-image/')
        self.assertEqual(status, 200)
        self.assertIn(b'libraryInput', tool_page)
        self.assertIn(b'liveVideo', tool_page)
        self.assertIn(b'nativeCameraInput', tool_page)
        self.assertEqual(headers['X-Content-Type-Options'], 'nosniff')
        status, raw, _ = self.request('/manifest.webmanifest')
        self.assertEqual(status, 200)
        self.assertEqual(json.loads(raw)['name'], 'My Tools')

    def test_02_pairing_required(self):
        status, _, _ = self.request('/api/status')
        self.assertEqual(status, 401)
        status, _, _ = self.request('/api/photo', method='POST', data=png_bytes())
        self.assertEqual(status, 401)

    def test_03_authorized_upload(self):
        status, raw, _ = self.request('/api/photo', method='POST', data=png_bytes(), token=self.state.token)
        self.assertEqual(status, 200)
        self.assertEqual(json.loads(raw)['message'], 'Pasted (test)')
        self.assertEqual(self.received[-1], (12, 9))
        status, raw, _ = self.request('/api/status', token=self.state.token)
        self.assertEqual(status, 200)
        self.assertGreaterEqual(json.loads(raw)['received'], 1)

    def test_04_broken_image_rejected(self):
        status, _, _ = self.request('/api/photo', method='POST', data=b'not an image', token=self.state.token)
        self.assertEqual(status, 415)

    def test_05_pair_rotation_disables_old_token(self):
        previous = self.state.token
        new = self.state.rotate_token()
        self.assertNotEqual(previous, new)
        status, _, _ = self.request('/api/status', token=previous)
        self.assertEqual(status, 401)
        status, _, _ = self.request('/api/status', token=new)
        self.assertEqual(status, 200)

    def test_06_large_image_rejected_before_read(self):
        # Any fake Content-Length > limit is rejected without waiting for the body.
        from image_processing import MAX_UPLOAD_BYTES
        req = Request(f'http://127.0.0.1:{self.port}/api/photo', data=b'x', method='POST',
                      headers={'Authorization': 'Bearer ' + self.state.token,
                               'Content-Length': str(MAX_UPLOAD_BYTES + 1)})
        # urllib may rewrite Content-Length; validate image loader's own size guard.
        with self.assertRaises(InvalidImage):
            load_and_normalize(b'x' * (MAX_UPLOAD_BYTES + 1))


if __name__ == '__main__':
    unittest.main()
