mod app;
mod complete;
mod history;
mod i18n;
mod i18n_es;
mod logger;
mod mapview;
mod output;
mod palette;
mod panels;
mod profile;
mod rules;
mod secrets;
mod settings;
mod sound;
mod tab;
mod theme;
mod voice;

use eframe::egui;
use muddie_core::Target;

use crate::profile::Profiles;
use crate::tab::Startup;

const USAGE: &str =
    "usage: muddie [--speech] [--script FILE]... [--profile NAME | HOST PORT [--tls] [--insecure]]";

fn parse_args(args: &[String]) -> Result<Startup, String> {
    let mut startup = Startup {
        profiles: Profiles::in_config_dir(),
        profile: None,
        target: None,
        speech: false,
        scripts: Vec::new(),
    };
    let (mut tls, mut insecure) = (false, false);
    let mut positional = Vec::new();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--tls" => tls = true,
            "--insecure" => insecure = true,
            "--speech" => startup.speech = true,
            "--script" => startup.scripts.push(args.next().ok_or(USAGE)?.into()),
            "--profile" => startup.profile = Some(args.next().ok_or(USAGE)?.clone()),
            flag if flag.starts_with("--") => return Err(USAGE.to_owned()),
            _ => positional.push(arg.as_str()),
        }
    }
    startup.target = match positional.as_slice() {
        [] if !tls && !insecure => None,
        [host, port] if startup.profile.is_none() => Some(Target {
            host: (*host).to_owned(),
            port: port.parse().map_err(|_| format!("invalid port: {port}"))?,
            tls,
            verify_certificate: !insecure,
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
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([640.0, 400.0]),
        ..Default::default()
    };
    eframe::run_native(
        "muddie",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc, startup)))),
    )
}
