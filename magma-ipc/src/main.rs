use ipc::generated::{workspaces::{Workspaces, Event as WorkspacesEvent}, magma_ipc::MagmaIpc};
use wayland_client::{Connection, Dispatch, protocol::wl_registry, QueueHandle, globals::{registry_queue_init, GlobalListContents}};

mod ipc;


struct State;

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
    fn event(
        _state: &mut Self,
        _: &wl_registry::WlRegistry,
        _event: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<State>,
    ) {}
}

impl Dispatch<MagmaIpc, ()> for State {
    fn event(
        _state: &mut Self,
        _proxy: &MagmaIpc,
        _event: <MagmaIpc as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}


impl Dispatch<Workspaces, ()> for State {
    fn event(
        State: &mut Self,
        _proxy: &Workspaces,
        event: WorkspacesEvent,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            WorkspacesEvent::ActiveWorkspace { id } => println!("{}", id),
        }
    }
}

fn main() {
    let conn = Connection::connect_to_env().unwrap();
    let mut event_queue = conn.new_event_queue();
    let qh = event_queue.handle();
    let (globals, _queue) = registry_queue_init::<State>(&conn).unwrap();
    let ipc: MagmaIpc = globals.bind::<MagmaIpc, _ ,_>(&qh, 1..=1, ()).unwrap();

    let category = ::std::env::args().nth(1);

    match category.as_ref().map(|s| &s[..]) {
        Some("workspace") => {
            ipc.workspaces(&qh, ());
        }
        Some(_) => {
            todo!()
        }
        None => {
            todo!()
        }
    }

    event_queue.roundtrip(&mut State).unwrap();
}
