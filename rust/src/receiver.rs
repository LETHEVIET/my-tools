//! Authenticated, bounded LAN HTTP receiver; the existing iPhone UI is embedded in the EXE.
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::io::{Cursor, Read};
use std::net::{IpAddr, UdpSocket};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::Engine;
use image::{DynamicImage, GenericImageView, ImageReader};
use qrcode::{Color, QrCode};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

use crate::win;

const MAX_UPLOAD: usize = 16 * 1024 * 1024;
const MAX_PIXELS: u64 = 36_000_000;

pub struct State {
    pub first_run: bool,
    token: Mutex<String>,
    token_file: PathBuf,
    pub base_url: String,
    pub auto_paste: AtomicBool,
    pub received: AtomicUsize,
    last_result: Mutex<String>,
    requests: Mutex<HashMap<IpAddr, VecDeque<Instant>>>,
    paste_lock: Mutex<()>,
}

fn random_token() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| e.to_string())?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes))
}

impl State {
    pub fn load(base_url: String) -> Result<Arc<Self>, String> {
        let app_dir = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
            .ok_or("Cannot find user profile directory")?
            .join("SnapToPrompt");
        fs::create_dir_all(&app_dir)
            .map_err(|e| format!("Cannot create settings directory: {e}"))?;
        let token_file = app_dir.join("pairing-token.txt");
        let token = fs::read_to_string(&token_file)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| {
                s.len() >= 40
                    && s.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            });
        let first_run = token.is_none();
        let token = match token {
            Some(t) => t,
            None => {
                let t = random_token()?;
                fs::write(&token_file, format!("{t}\n"))
                    .map_err(|e| format!("Cannot save pairing secret: {e}"))?;
                t
            }
        };
        Ok(Arc::new(Self {
            token: Mutex::new(token),
            token_file,
            base_url,
            first_run,
            auto_paste: AtomicBool::new(true),
            received: AtomicUsize::new(0),
            last_result: Mutex::new("Waiting for a photo".into()),
            requests: Mutex::new(HashMap::new()),
            paste_lock: Mutex::new(()),
        }))
    }

    pub fn pairing_url(&self) -> String {
        format!("{}/#pair={}", self.base_url, self.token.lock().unwrap())
    }

    pub fn rotate(&self) -> Result<(), String> {
        let token = random_token()?;
        // Only invalidate the old token after the new one is safely persisted.
        fs::write(&self.token_file, format!("{token}\n"))
            .map_err(|e| format!("Cannot save new pairing secret: {e}"))?;
        *self.token.lock().unwrap() = token;
        self.requests.lock().unwrap().clear();
        Ok(())
    }

    fn authenticated(&self, auth: &str) -> bool {
        let candidate = auth.strip_prefix("Bearer ").unwrap_or("");
        let token = self.token.lock().unwrap();
        // Constant-work comparison for equal-length secrets.
        let a = candidate.as_bytes();
        let b = token.as_bytes();
        if a.len() != b.len() {
            return false;
        }
        a.iter()
            .zip(b.iter())
            .fold(0u8, |diff, (&x, &y)| diff | (x ^ y))
            == 0
    }

    fn allow_upload(&self, ip: IpAddr) -> bool {
        let now = Instant::now();
        let mut requests = self.requests.lock().unwrap();
        let recent = requests.entry(ip).or_default();
        while recent
            .front()
            .is_some_and(|t| now.duration_since(*t) >= Duration::from_secs(60))
        {
            recent.pop_front();
        }
        if recent.len() >= 12 {
            return false;
        }
        recent.push_back(now);
        true
    }
}

pub fn local_ip() -> String {
    UdpSocket::bind("0.0.0.0:0")
        .and_then(|socket| {
            socket.connect("1.1.1.1:80")?;
            Ok(socket.local_addr()?.ip().to_string())
        })
        .unwrap_or_else(|_| "127.0.0.1".into())
}

fn header(key: &str, value: &str) -> Header {
    Header::from_bytes(key.as_bytes(), value.as_bytes()).unwrap()
}

fn respond(req: Request, status: u16, mime: &str, body: Vec<u8>) {
    let response = Response::from_data(body)
        .with_status_code(StatusCode(status))
        .with_header(header("Content-Type", mime))
        .with_header(header("Cache-Control", "no-store"))
        .with_header(header("X-Content-Type-Options", "nosniff"))
        .with_header(header("Referrer-Policy", "no-referrer"))
        .with_header(header("Content-Security-Policy", "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' blob: data:; connect-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'"));
    let _ = req.respond(response);
}

fn reply(req: Request, code: u16, message: &str) {
    let escaped = message
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', " ");
    respond(
        req,
        code,
        "application/json; charset=utf-8",
        format!("{{\"error\":\"{escaped}\"}}").into_bytes(),
    );
}

fn asset(path: &str) -> Option<(&'static [u8], &'static str)> {
    let res = match path {
        "/" | "/index.html" => (
            include_bytes!("../../web/index.html").as_slice(),
            "text/html; charset=utf-8",
        ),
        "/app.js" => (
            include_bytes!("../../web/app.js").as_slice(),
            "text/javascript; charset=utf-8",
        ),
        "/app.css" => (
            include_bytes!("../../web/app.css").as_slice(),
            "text/css; charset=utf-8",
        ),
        "/sw.js" => (
            include_bytes!("../../web/sw.js").as_slice(),
            "text/javascript; charset=utf-8",
        ),
        "/manifest.webmanifest" => (
            include_bytes!("../../web/manifest.webmanifest").as_slice(),
            "application/manifest+json",
        ),
        "/icons/icon-192.png" => (
            include_bytes!("../../web/icons/icon-192.png").as_slice(),
            "image/png",
        ),
        "/icons/icon-512.png" => (
            include_bytes!("../../web/icons/icon-512.png").as_slice(),
            "image/png",
        ),
        _ => return None,
    };
    Some(res)
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn pairing_page(state: &State) -> String {
    let url = state.pairing_url();
    let code = QrCode::new(url.as_bytes()).expect("pairing URL should fit into a QR code");
    let mut svg = format!("<svg viewBox=\"0 0 {} {}\" width=\"272\" height=\"272\" xmlns=\"http://www.w3.org/2000/svg\"><rect width=\"100%\" height=\"100%\" fill=\"white\"/>", code.width() + 8, code.width() + 8);
    for y in 0..code.width() {
        for x in 0..code.width() {
            if code[(x, y)] == Color::Dark {
                svg.push_str(&format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"1\" height=\"1\" fill=\"#102032\"/>",
                    x + 4,
                    y + 4
                ));
            }
        }
    }
    svg.push_str("</svg>");
    format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta name="referrer" content="no-referrer"><title>Snap to Prompt pairing</title><style>body{{font:16px system-ui;background:#101827;color:#eff8f9;margin:0;min-height:100vh;display:grid;place-items:center}}main{{text-align:center;max-width:440px;padding:25px}}svg{{background:white;padding:12px;border-radius:12px;max-width:90vw}}a{{color:#45dfb0;overflow-wrap:anywhere}}p{{color:#b5c4d5}}button{{background:#24384c;color:white;border:0;border-radius:8px;padding:12px 18px;cursor:pointer}}</style></head><body><main><h1>Snap to Prompt</h1><p>Scan this once with your iPhone camera. Leave this receiver running in the Windows tray.</p>{svg}<p><a href="{link}">Open pairing URL</a></p><p>After pairing, focus the AI prompt on Windows and take a photo on iPhone.</p></main></body></html>"#,
        link = escape_html(&url)
    )
}

fn normalized_image(bytes: &[u8]) -> Result<DynamicImage, &'static str> {
    if bytes.is_empty() || bytes.len() > MAX_UPLOAD {
        return Err("Invalid image size");
    }
    let dimensions = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| "Unsupported image format")?
        .into_dimensions()
        .map_err(|_| "Unsupported or corrupt image")?;
    if u64::from(dimensions.0) * u64::from(dimensions.1) > MAX_PIXELS {
        return Err("Photo resolution is too large");
    }
    let mut image = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| "Unsupported image format")?
        .decode()
        .map_err(|_| "Unsupported or corrupt JPEG/PNG")?;
    if let Ok(exif) = exif::Reader::new().read_from_container(&mut Cursor::new(bytes)) {
        if let Some(orientation) = exif
            .get_field(exif::Tag::Orientation, exif::In::PRIMARY)
            .and_then(|field| field.value.get_uint(0))
        {
            image = match orientation {
                2 => image.fliph(),
                3 => image.rotate180(),
                4 => image.flipv(),
                5 => image.rotate90().fliph(),
                6 => image.rotate90(),
                7 => image.rotate270().fliph(),
                8 => image.rotate270(),
                _ => image,
            };
        }
    }
    let (w, h) = image.dimensions();
    if w.max(h) > 2400 {
        image = image.thumbnail(2400, 2400);
    }
    Ok(image)
}

fn handle(mut req: Request, state: &Arc<State>) {
    let path = req.url().split('?').next().unwrap_or("");
    if req.method() == &Method::Get && path == "/pair" {
        let is_local = req
            .remote_addr()
            .is_some_and(|addr| addr.ip().is_loopback());
        if !is_local {
            return reply(req, 403, "Pairing is available on this PC only");
        }
        return respond(
            req,
            200,
            "text/html; charset=utf-8",
            pairing_page(state).into_bytes(),
        );
    }
    if req.method() == &Method::Get && path == "/api/status" {
        let auth = req
            .headers()
            .iter()
            .find(|h| h.field.equiv("Authorization"))
            .map(|h| h.value.as_str())
            .unwrap_or("");
        if !state.authenticated(auth) {
            return reply(req, 401, "Pairing expired. Scan the new QR code.");
        }
        let last = state.last_result.lock().unwrap().clone();
        let body = format!(
            "{{\"paired\":true,\"auto_paste\":{},\"received\":{},\"last_result\":\"{}\"}}",
            state.auto_paste.load(Ordering::Relaxed),
            state.received.load(Ordering::Relaxed),
            last
        );
        return respond(
            req,
            200,
            "application/json; charset=utf-8",
            body.into_bytes(),
        );
    }
    if req.method() == &Method::Post && path == "/api/photo" {
        let auth = req
            .headers()
            .iter()
            .find(|h| h.field.equiv("Authorization"))
            .map(|h| h.value.as_str())
            .unwrap_or("");
        if !state.authenticated(auth) {
            return reply(req, 401, "Pairing expired. Scan the PC QR code again.");
        }
        let ip = req
            .remote_addr()
            .map(|a| a.ip())
            .unwrap_or(IpAddr::from([127, 0, 0, 1]));
        if !state.allow_upload(ip) {
            return reply(req, 429, "Too many photos. Try again shortly.");
        }
        if req.body_length().unwrap_or(0) == 0 {
            return reply(req, 411, "No photo attached");
        }
        if req.body_length().unwrap_or(0) > MAX_UPLOAD {
            return reply(req, 413, "Maximum photo size is 16 MB");
        }
        let mut bytes = Vec::new();
        if req
            .as_reader()
            .take((MAX_UPLOAD + 1) as u64)
            .read_to_end(&mut bytes)
            .is_err()
        {
            return reply(req, 400, "Incomplete photo upload");
        }
        if bytes.len() > MAX_UPLOAD {
            return reply(req, 413, "Maximum photo size is 16 MB");
        }
        let image = match normalized_image(&bytes) {
            Ok(img) => img,
            Err(msg) => return reply(req, 415, msg),
        };
        let (width, height) = image.dimensions();
        let _exclusive = state.paste_lock.lock().unwrap();
        let result = match win::copy_and_paste(&image, state.auto_paste.load(Ordering::Relaxed)) {
            Ok(msg) => msg,
            Err(msg) => return reply(req, 409, &msg),
        };
        state.received.fetch_add(1, Ordering::Relaxed);
        *state.last_result.lock().unwrap() = result.into();
        let body = format!(
            "{{\"ok\":true,\"message\":\"{result}\",\"width\":{width},\"height\":{height}}}"
        );
        return respond(
            req,
            200,
            "application/json; charset=utf-8",
            body.into_bytes(),
        );
    }
    if req.method() == &Method::Get {
        if let Some((bytes, mime)) = asset(path) {
            return respond(req, 200, mime, bytes.to_vec());
        }
    }
    reply(req, 404, "Not found");
}

pub fn serve(server: Server, state: Arc<State>) {
    for request in server.incoming_requests() {
        handle(request, &state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_photo() {
        assert!(normalized_image(b"not an image").is_err());
    }
    #[test]
    fn assets_match_iphone_protocol() {
        assert!(std::str::from_utf8(asset("/app.js").unwrap().0)
            .unwrap()
            .contains("/api/photo"));
        assert!(std::str::from_utf8(asset("/app.js").unwrap().0)
            .unwrap()
            .contains("getUserMedia"));
        assert!(std::str::from_utf8(asset("/index.html").unwrap().0)
            .unwrap()
            .contains("libraryInput"));
        assert!(asset("/icons/icon-192.png").unwrap().0.len() > 100);
    }
    #[test]
    fn decodes_valid_photo_and_strips_it_to_rgb() {
        let source = image::RgbImage::from_pixel(12, 9, image::Rgb([240, 20, 10]));
        let mut data = Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(source)
            .write_to(&mut data, image::ImageFormat::Png)
            .unwrap();
        let output = normalized_image(&data.into_inner()).unwrap();
        assert_eq!(output.dimensions(), (12, 9));
        assert_eq!(output.to_rgb8().get_pixel(0, 0).0, [240, 20, 10]);
    }
}
