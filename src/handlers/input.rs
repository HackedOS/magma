use tracing::info;

use crate::{
    config::Action,
    state::{Backend, HoloState},
};

impl<BackendData: Backend> HoloState<BackendData> {
    pub fn handle_action(&mut self, action: Action) {
        match action {
            Action::Quit => self.loop_signal.stop(),
            Action::Debug => todo!(),
            Action::Close => {
                if let Some(d) = self
                    .workspaces
                    .current()
                    .window_under(self.seat.get_pointer().unwrap().current_location())
                {
                    d.0.toplevel().send_close()
                }
            }

            Action::Workspace(id) => self.workspaces.activate(id),
            Action::ToggleWindowFloating => todo!(),
            Action::Spawn(command) => {
                if let Err(err) = std::process::Command::new("/bin/sh")
                    .arg("-c")
                    .arg(command.clone())
                    .spawn()
                {
                    info!("{} {} {}", err, "Failed to spawn \"{}\"", command);
                }
            }
        }
    }
}
