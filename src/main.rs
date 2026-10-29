//! Hokum Sound: audio control for Linux on PipeWire, drawn with Rune. One of the Hokum tools.
//!
//! ```sh
//! sound                        # open the window
//! sound --mock                 # the window on fake devices and apps (no PipeWire)
//! sound service                # stay running in the background (tray, hotkeys; later)
//! sound check                  # report problems in config.kdl
//! sound update [--install]     # check for a new build now (and install it)
//! sound --version
//! sound --snap /tmp/sound.png --scene mock   # headless render on the mock backend
//! ```
//!
//! `crate::model` is the contract between the backend (`crate::audio`, PipeWire;
//! `crate::mock`, fake) and the window (`crate::ui`).

mod audio;
// Parts of the contract the backend and the window don't use yet.
#[allow(dead_code)]
mod config;
mod mock;
#[allow(dead_code)]
mod model;
mod ui;
#[allow(dead_code)]
mod update;

const HELP: &str = "usage: sound [service | check | update] [--mock] [--version] [--help]
  sound                  open the window
    --mock               use fake devices and apps instead of PipeWire
                         (or HOKUM_SOUND_BACKEND=mock)
  sound service          stay running in the background (tray and hotkeys; not yet)
  sound check            print problems in ~/.config/hokum-sound/config.kdl;
                         exits non-zero if there are any
  sound update           check for a new build on this build's channel now
    --install            and install it, for a plain writable install
  --version              print the version
Rune's tooling flags (--snap, --scene, --size, --theme, ...) render headless on the mock
backend.";

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.iter().any(|a| a == "--version" || a == "-V") {
        println!("sound {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if argv.iter().any(|a| a == "--help" || a == "-h") {
        println!("{HELP}");
        return;
    }
    match argv.first().map(String::as_str) {
        Some("check") => std::process::exit(if config::check() { 0 } else { 1 }),
        Some("update") => std::process::exit(update::cli(&argv)),
        Some("service") => {
            eprintln!("sound: the background service is not implemented yet");
            std::process::exit(1);
        }
        _ => {}
    }
    let headless = argv.iter().any(|a| {
        matches!(a.as_str(), "--snap" | "--tree" | "--trace" | "--frames" | "--strip" | "--list-scenes" | "--first-pixels")
    });
    if headless {
        rune::App::new(ui::spec()).run();
        return;
    }
    let mock = argv.iter().any(|a| a == "--mock") || std::env::var("HOKUM_SOUND_BACKEND").is_ok_and(|b| b == "mock");
    let source = if mock { model::Source::Mock } else { model::Source::PipeWire };
    match model::start(source) {
        Ok(audio) => ui::run(audio),
        Err(e) => {
            eprintln!("sound: {e}");
            std::process::exit(1);
        }
    }
}
