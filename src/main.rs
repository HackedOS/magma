use state::{CalloopData, HoloState};
use tracing::{error, info};

use crate::backends::winit;

mod backends;
mod config;
mod handlers;
mod input;
mod state;
mod utils;

static POSSIBLE_BACKENDS: &[&str] = &[
    "--winit : Run holo as a X11 or Wayland client using winit.",
    "--tty-udev : Run holo as a tty udev client (requires root if without logind).",
];
fn main() {
    if let Ok(env_filter) = tracing_subscriber::EnvFilter::try_from_default_env() {
        tracing_subscriber::fmt().with_env_filter(env_filter).init();
    } else {
        tracing_subscriber::fmt().init();
    }

    let arg = ::std::env::args().nth(1);
    match arg.as_ref().map(|s| &s[..]) {
        Some("--winit") => {
            info!("Starting holown with winit backend");
            winit::init_winit();
        }
        Some("--tty-udev") => {
            info!("Starting holo on a tty using udev");
            // udev::init_udev();
        }
        Some(other) => {
            error!("Unknown backend: {}", other);
        }
        None => {
            println!("USAGE: holo --backend");
            println!();
            println!("Possible backends are:");
            for b in POSSIBLE_BACKENDS {
                println!("\t{}", b);
            }
        }
    }

    info!("Holo is shutting down");
}
