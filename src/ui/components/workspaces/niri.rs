use {
    super::{Ws, WORKSPACE_COUNT},
    niri_ipc::{socket::Socket, Action, Event, Request, Response, WorkspaceReferenceArg},
};

#[derive(Clone, Copy, Debug)]
struct NiriWs {
    id: u64,
    idx: u8,
    is_active: bool,
    is_focused: bool,
}

pub(super) fn event_loop(tx: &async_channel::Sender<Vec<Ws>>) -> Result<(), String> {
    ipc_loop(tx).map_err(|e| e.to_string())
}

fn ipc_loop(tx: &async_channel::Sender<Vec<Ws>>) -> std::io::Result<()> {
    let mut socket = Socket::connect()?;
    let reply = socket.send(Request::EventStream)?;
    if !matches!(reply, Ok(Response::Handled)) {
        crate::log::warn(
            "workspaces",
            &format!("unexpected reply to EventStream: {reply:?}"),
        );
    }
    let mut read_event = socket.read_events();

    let mut state: Vec<NiriWs> = Vec::new();

    loop {
        match read_event()? {
            Event::WorkspacesChanged { workspaces } => {
                state = workspaces
                    .into_iter()
                    .map(|w| {
                        NiriWs {
                            id: w.id,
                            idx: w.idx,
                            is_active: w.active_window_id.is_some(),
                            is_focused: w.is_focused,
                        }
                    })
                    .collect();
            },
            Event::WorkspaceActivated { id, focused } => {
                for w in state.iter_mut() {
                    if focused {
                        w.is_focused = w.id == id;
                    }
                }
            },
            Event::WorkspaceActiveWindowChanged {
                workspace_id,
                active_window_id,
            } => {
                if let Some(ws) = state.iter_mut().find(|ws| ws.id == workspace_id) {
                    ws.is_active = active_window_id.is_some();
                }
            },
            _ => {},
        }

        let snap: Vec<Ws> = (1..=WORKSPACE_COUNT)
            .map(|idx| {
                state.iter().find(|wks| wks.idx == idx).map_or_else(
                    || {
                        Ws {
                            id: u64::from(idx),
                            label: idx.to_string(),
                            is_active: false,
                            is_focused: false,
                        }
                    },
                    |wks| {
                        Ws {
                            id: wks.id,
                            label: wks.idx.to_string(),
                            is_active: wks.is_active,
                            is_focused: wks.is_focused,
                        }
                    },
                )
            })
            .collect();

        let _ = tx.send_blocking(snap);
    }
}

pub(super) fn focus_workspace(id: u64) {
    let mut socket = match Socket::connect() {
        Ok(socket) => socket,
        Err(e) => {
            crate::log::warn("workspaces", &format!("could not connect to niri IPC: {e}"));
            return;
        },
    };
    if let Err(e) = socket.send(Request::Action(Action::FocusWorkspace {
        reference: WorkspaceReferenceArg::Id(id),
    })) {
        crate::log::warn("workspaces", &format!("failed to focus workspace: {e}"));
    }
}
