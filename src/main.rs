use backends::winit::init_winit;
use smithay::reexports::{calloop::EventLoop, wayland_server::Display};
use state::{CalloopData, HoloState};

mod backends;
mod config;
mod handlers;
mod input;
mod state;
mod utils;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut event_loop: EventLoop<CalloopData> = EventLoop::try_new()?;

    let mut display: Display<HoloState> = Display::new()?;
    let state = HoloState::new(&mut event_loop, &mut display);

    let mut data = CalloopData {
        display,
        state: state,
    };

    init_winit(&mut event_loop, &mut data)?;

    std::process::Command::new("alacritty").spawn().ok();

    event_loop.run(None, &mut data, move |_| {
        // HoloWM is running
    })?;

    Ok(())
}
