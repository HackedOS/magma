use crate::{config::Action, state::HoloState};

impl HoloState {
    pub fn handle_action(&self, action: Action) {
        match action {
            Action::Terminate => todo!(),
            Action::Debug => todo!(),
            Action::Close => todo!(),
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
