use crate::{config::Action, state::HoloState};

impl HoloState {
    pub fn handle_action(&self, action: Action) {
        match action {
            Action::Terminate => self.loop_signal.stop(),
            Action::Debug => todo!(),
            Action::Close => match self
                .workspace
                .window_under(self.seat.get_pointer().unwrap().current_location())
            {
                Some(d) => d.0.toplevel().send_close(),
                None => {}
            },

            Action::Workspace(_) => todo!(),
            Action::ToggleWindowFloating => todo!(),
            Action::Spawn(command) => {
                if let Err(err) = std::process::Command::new("/bin/sh")
                    .arg("-c")
                    .arg(command.clone())
                    .spawn()
                {
                    println!("{} {} {}", err, "Failed to spawn \"{}\"", command);
                }
            }
        }
    }
}
