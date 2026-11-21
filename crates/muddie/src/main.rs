// A window of its own on Windows, with no console behind it.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod autosave;
mod brand;
mod bundled;
mod complete;
mod debug;
mod history;
mod i18n;
mod keypad;
mod logger;
mod luau;
mod mapfetch;
mod mapview;
mod media;
mod output;
mod packages;
mod palette;
mod panels;
mod profile;
mod rules;
mod secrets;
mod settings;
mod shortcuts;
mod syntax;
mod tab;
mod theme;
mod voice;
mod widgets;

use std::sync::{Arc, Mutex};

use eframe::egui;
use muddie_core::Target;
use winit::event_loop::EventLoop;

use crate::profile::Profiles;
use crate::tab::Startup;

const USAGE: &str =
    "usage: muddie [--speech] [--script FILE]... [--profile NAME | HOST PORT [--tls] [--insecure]]
              [--debug]
       muddie --version

  --debug  shows on the standard output everything that happens: what goes over
           the protocols (GMCP<, GMCP>, MSDP, TELNET...), the game's text, what
           is sent, what the client says, the scripts' errors and logs, and
           the map. Passwords are never shown. To keep it: muddie --debug | tee";

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // What it is, and what it can speak through, without a window: to
    // check that it was built and installed whole.
    if args.iter().any(|arg| arg == "--version") {
        println!("muddie {}", env!("CARGO_PKG_VERSION"));
        println!("speech: {}", muddie_speech::built_in().join(", "));
        return Ok(());
    }
    // Everything that happens, on the standard output, if asked.
    if args.iter().any(|arg| arg == "--debug") {
        debug::start();
    }
    let args: Vec<String> = args.into_iter().filter(|arg| arg != "--debug").collect();
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
            .with_title("Muddie")
            .with_maximized(true)
            .with_app_id(brand::APP_ID)
            .with_icon(std::sync::Arc::new(eframe::icon_data::from_png_bytes(
                brand::ICON_PNG,
            )?))
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([640.0, 400.0]),
        ..Default::default()
    };
    // The application runs inside an event loop of our own, so that the
    // keys of the numeric keypad can be told apart on the way in.
    let event_loop = EventLoop::<eframe::UserEvent>::with_user_event().build()?;
    let pressed = keypad::Pressed::default();
    let ctx = Arc::new(Mutex::new(None));
    let (for_app, ctx_for_app) = (Arc::clone(&pressed), Arc::clone(&ctx));
    let app = eframe::create_native(
        "muddie",
        options,
        Box::new(move |cc| {
            *ctx_for_app.lock().expect("context lock") = Some(cc.egui_ctx.clone());
            Ok(Box::new(app::App::new(cc, startup, for_app)))
        }),
        &event_loop,
    );
    event_loop.run_app(&mut keypad::Watcher {
        app,
        pressed,
        ctx,
        composing: false,
    })?;
    Ok(())
}
