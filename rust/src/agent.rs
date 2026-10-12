//! Small background-agent entry point shared by the standalone development crate
//! and the single-file My Tools desktop executable.
use crate::{receiver, win};

pub fn run() -> Result<(), String> {
    let mut port = 8765u16;
    let mut host: Option<String> = None;
    let mut public_url: Option<String> = None;
    let mut show = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--agent" | "--minimized" | "--headless-test" => {}
            "--port" => {
                port = args
                    .next()
                    .ok_or("Missing --port value")?
                    .parse()
                    .map_err(|_| "Invalid port")?
            }
            "--host" => host = Some(args.next().ok_or("Missing --host value")?),
            "--public-url" => {
                let value = args.next().ok_or("Missing --public-url value")?;
                if !value.starts_with("https://") {
                    return Err("--public-url must start with https://".to_owned());
                }
                public_url = Some(value.trim_end_matches('/').to_owned());
            }
            "--show" => show = true,
            unknown => return Err(format!("Unknown background agent argument: {unknown}")),
        }
    }

    let host = host.unwrap_or_else(receiver::local_ip);
    let base_url = public_url.unwrap_or_else(|| format!("http://{host}:{port}"));
    let state = receiver::State::load(base_url)?;
    let http = tiny_http::Server::http(("0.0.0.0", port)).map_err(|error| {
        format!("Cannot listen on port {port}: {error}. Close the old tray agent first.")
    })?;
    let running = state.clone();
    std::thread::Builder::new()
        .name("photo-receiver".to_owned())
        .spawn(move || receiver::serve(http, running))
        .map_err(|error| error.to_string())?;
    let show = show || state.first_run;
    win::run_tray(state, port, show)
}
