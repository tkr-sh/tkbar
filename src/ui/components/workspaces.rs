use {
    crate::conf::CONFIG,
    gtk::{Align, Box as GtkBox, Button, glib, prelude::*},
    gtk4 as gtk,
    niri_ipc::{Action, Event, Request, Response, WorkspaceReferenceArg, socket::Socket},
    std::thread,
};

const WORKSPACE_COUNT: u8 = 10;

#[derive(Clone, Copy, Debug)]
struct NiriWs {
    id: u64,
    idx: u8,
    is_active: bool,
    is_focused: bool,
}

pub fn workspaces() -> GtkBox {
    let container = GtkBox::new(CONFIG.position.orientation(), 4);
    container.add_css_class("workspaces");

    let (tx, rx) = async_channel::unbounded::<Vec<NiriWs>>();

    thread::spawn(move || {
        if let Err(e) = event_loop(&tx) {
            crate::log::warn("workspaces", &format!("niri IPC error: {e}"));
        }
    });

    glib::spawn_future_local(glib::clone!(
        #[weak]
        container,
        #[upgrade_or_default]
        async move {
            while let Ok(mut list) = rx.recv().await {
                list.sort_by_key(|w| w.idx);

                while let Some(child) = container.first_child() {
                    container.remove(&child);
                }

                for ws in &list {
                    let btn = Button::with_label(&ws.idx.to_string());
                    btn.add_css_class("workspace");
                    if ws.is_focused {
                        btn.add_css_class("current");
                    }
                    if !ws.is_active {
                        btn.add_css_class("inactive");
                    }
                    btn.set_valign(Align::Center);
                    btn.set_halign(Align::Center);

                    let id = ws.id;
                    btn.connect_clicked(move |_| focus_workspace(id));
                    container.append(&btn);
                }
            }
        }
    ));

    container
}

fn event_loop(tx: &async_channel::Sender<Vec<NiriWs>>) -> std::io::Result<()> {
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


        let new_state = (1..=WORKSPACE_COUNT).map(|idx| {
            state
                .iter()
                .find(|wks| wks.idx == idx)
                .copied()
                .unwrap_or_else(|| {
                    NiriWs {
                        id: u64::from(idx),
                        idx,
                        is_active: false,
                        is_focused: false,
                    }
                })
        });

        let _ = tx.send_blocking(new_state.collect());
    }
}

fn focus_workspace(id: u64) {
    thread::spawn(move || {
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
    });
}
