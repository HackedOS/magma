use super::generated::workspaces::Event;

impl Into<String> for Event {
    fn into(self) -> String {
        match self {
            Event::ActiveWorkspace { id: _ } => "active_workspace".to_owned(),
        }
    }
}