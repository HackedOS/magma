use smithay::reexports::{calloop::EventLoop, wayland_server::Display};
use state::{CalloopData, HoloState};

mod backends;
mod handlers;
mod input;
mod state;
mod utils;

fn main() {
    if let Ok(env_filter) = tracing_subscriber::EnvFilter::try_from_default_env() {
        tracing_subscriber::fmt().with_env_filter(env_filter).init();
    } else {
        tracing_subscriber::fmt().init();
    }

    let mut event_loop: EventLoop<CalloopData> = EventLoop::try_new().unwrap();

    let mut display: Display<HoloState> = Display::new().unwrap();
    let state = HoloState::new(&mut event_loop, &mut display);

    let mut data = CalloopData { state, display };

    crate::backends::winit::init_winit(&mut event_loop, &mut data).unwrap();

    std::process::Command::new("alacritty").spawn().ok();

    event_loop
        .run(None, &mut data, move |_| {
            // HoloState is running
        })
        .unwrap();
}
