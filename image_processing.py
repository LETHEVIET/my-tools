"""Image validation and Windows CF_DIB clipboard encoding."""
from __future__ import annotations

from io import BytesIO
import struct
from PIL import Image, ImageOps, UnidentifiedImageError

MAX_UPLOAD_BYTES = 16 * 1024 * 1024
MAX_PIXELS = 36_000_000
MAX_OUTPUT_EDGE = 2400
Image.MAX_IMAGE_PIXELS = MAX_PIXELS


class InvalidImage(ValueError):
    pass


def load_and_normalize(data: bytes) -> Image.Image:
    """Decode a photo, apply camera orientation, resize, and strip metadata."""
    if not data:
        raise InvalidImage("Empty upload")
    if len(data) > MAX_UPLOAD_BYTES:
        raise InvalidImage("Photo is larger than 16 MB")
    try:
        with Image.open(BytesIO(data)) as source:
            if source.width * source.height > MAX_PIXELS:
                raise InvalidImage("Photo resolution is too large")
            source.load()
            corrected = ImageOps.exif_transpose(source)
            if corrected.mode in ("RGBA", "LA") or (corrected.mode == "P" and "transparency" in corrected.info):
                rgb = Image.new("RGB", corrected.size, "white")
                rgb.paste(corrected.convert("RGBA"), mask=corrected.convert("RGBA").getchannel("A"))
            else:
                rgb = corrected.convert("RGB")
            rgb.thumbnail((MAX_OUTPUT_EDGE, MAX_OUTPUT_EDGE), Image.Resampling.LANCZOS)
            return rgb.copy()
    except (UnidentifiedImageError, OSError, Image.DecompressionBombError, ValueError) as exc:
        if isinstance(exc, InvalidImage):
            raise
        raise InvalidImage("Unsupported or damaged image. Try JPEG or PNG") from exc


def image_to_dib(image: Image.Image) -> bytes:
    """Make CF_DIB (24-bit BGR, DWORD-aligned bottom-up scanlines) for Win32."""
    rgb = image.convert("RGB")
    width, height = rgb.size
    stride = ((width * 3 + 3) // 4) * 4
    raw = rgb.tobytes("raw", "BGR")
    rows = [raw[y * width * 3:(y + 1) * width * 3] + b"\0" * (stride - width * 3)
            for y in range(height - 1, -1, -1)]
    bits = b"".join(rows)
    # BITMAPINFOHEADER: size, width, height, planes, bpp, compression, image bytes, x/y ppm, colors
    header = struct.pack("<IiiHHIIiiII", 40, width, height, 1, 24, 0, len(bits), 0, 0, 0, 0)
    return header + bits
