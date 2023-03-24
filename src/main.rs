use state::{CalloopData, HoloState};
use tracing::{error, info};

use crate::backends::{udev, winit};

mod backends;
mod config;
mod handlers;
mod input;
mod state;
mod utils;

static POSSIBLE_BACKENDS: &[&str] = &[
    "--winit : Run holowm as a X11 or Wayland client using winit.",
    "--tty-udev : Run holowm as a tty udev client (requires root if without logind).",
];
fn main() {
    if let Ok(env_filter) = tracing_subscriber::EnvFilter::try_from_default_env() {
        tracing_subscriber::fmt()
            .compact()
            .with_env_filter(env_filter)
            .init();
    } else {
        tracing_subscriber::fmt().compact().init();
    }

    let arg = ::std::env::args().nth(1);
    match arg.as_ref().map(|s| &s[..]) {
        Some("--winit") => {
            info!("Starting holown with winit backend");
            winit::init_winit();
        }
        Some("--tty-udev") => {
            info!("Starting holowm on a tty using udev");
            udev::init_udev();
        }
        Some(other) => {
            error!("Unknown backend: {}", other);
        }
        None => {
            println!("USAGE: holowm --backend");
            println!();
            println!("Possible backends are:");
            for b in POSSIBLE_BACKENDS {
                println!("\t{}", b);
            }
        }
    }

    info!("HoloWM is shutting down");
}
