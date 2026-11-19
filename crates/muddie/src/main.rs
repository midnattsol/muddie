mod app;
mod history;
mod mapview;
mod output;
mod palette;
mod panels;
mod voice;

use std::path::PathBuf;

use eframe::egui;
use muddie_core::Target;

use crate::app::Startup;

const USAGE: &str = "usage: muddie [--speech] [--script FILE]... [HOST PORT [--tls]]";

/// The user's startup script, run before any `--script` file if it exists.
fn init_script() -> Option<PathBuf> {
    let config_dir = std::env::var_os("XDG_CONFIG_HOME")
        .or_else(|| std::env::var_os("APPDATA"))
        .map(PathBuf::from)
        .or_else(|| Some(PathBuf::from(std::env::var_os("HOME")?).join(".config")))?;
    let path = config_dir.join("muddie").join("init.luau");
    path.is_file().then_some(path)
}

fn parse_args(args: &[String]) -> Result<Startup, String> {
    let mut startup = Startup {
        target: None,
        speech: false,
        scripts: init_script().into_iter().collect(),
    };
    let mut tls = false;
    let mut positional = Vec::new();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--tls" => tls = true,
            "--speech" => startup.speech = true,
            "--script" => startup.scripts.push(args.next().ok_or(USAGE)?.into()),
            flag if flag.starts_with("--") => return Err(USAGE.to_owned()),
            _ => positional.push(arg.as_str()),
        }
    }
    startup.target = match positional.as_slice() {
        [] if !tls => None,
        [host, port] => Some(Target {
            host: (*host).to_owned(),
            port: port.parse().map_err(|_| format!("invalid port: {port}"))?,
            tls,
        }),
        _ => return Err(USAGE.to_owned()),
    };
    Ok(startup)
}

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let startup = match parse_args(&args) {
        Ok(startup) => startup,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };

    // Sessions run on this runtime; the UI keeps the main thread.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("failed to start the async runtime");
    let _guard = runtime.enter();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("muddie")
            .with_app_id("muddie")
            .with_inner_size([960.0, 640.0]),
        ..Default::default()
    };
    eframe::run_native(
        "muddie",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc, startup)))),
    )
}
