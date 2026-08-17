use {
    super::Ws,
    crate::conf::CONFIG,
    niri_ipc::{Action, Event, Request, Response, WorkspaceReferenceArg, socket::Socket},
};

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

    let mut state: Vec<Ws> = Vec::new();

    loop {
        match read_event()? {
            Event::WorkspacesChanged { workspaces } => {
                state = workspaces
                    .into_iter()
                    .map(|w| {
                        Ws {
                            id: w.id,
                            idx: w.idx,
                            label: if CONFIG.security.should_allow_workspace_label &&
                                let Some(name) = w.name
                            {
                                name
                            } else {
                                w.idx.to_string()
                            },
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

        if CONFIG.behaviour.should_show_empty_workspace {
            for idx in 1..=CONFIG.behaviour.workspace_count {
                if state.iter().all(|wks| wks.idx != idx) {
                    state.push(Ws {
                        id: u64::from(idx),
                        idx,
                        label: idx.to_string(),
                        is_active: false,
                        is_focused: false,
                    });
                }
            }
        }

        state.sort_unstable_by_key(|wks| wks.idx);

        let _ = tx.send_blocking(state.clone());
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
