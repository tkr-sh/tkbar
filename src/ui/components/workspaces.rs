use {
    gtk::{glib, prelude::*, Box as GtkBox, Button, Orientation},
    gtk4 as gtk,
    niri_ipc::{socket::Socket, Action, Event, Request, Response, WorkspaceReferenceArg},
    std::thread,
};

#[derive(Clone, Debug)]
struct NiriWs {
    id: u64,
    idx: u8,
    output: Option<String>,
    is_active: bool,
    is_focused: bool,
}

pub fn workspaces() -> GtkBox {
    let container = GtkBox::new(Orientation::Vertical, 4);
    container.add_css_class("workspaces");

    let (tx, rx) = async_channel::unbounded::<Vec<NiriWs>>();

    thread::spawn(move || {
        if let Err(e) = event_loop(&tx) {
            eprintln!("niri IPC error: {e}");
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
                        btn.add_css_class("wokrspace-current");
                    } else if !ws.is_active {
                        btn.add_css_class("workspace-inactive");
                    }

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
        eprintln!("niri: unexpected reply to EventStream: {reply:?}");
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
                            output: w.output,
                            is_active: w.active_window_id.is_some(),
                            is_focused: w.is_focused,
                        }
                    })
                    .collect();
                let _ = tx.send_blocking(state.clone());
            },
            Event::WorkspaceActivated { id, focused } => {
                for w in state.iter_mut() {
                    println!("{w:#?} {focused:#?} {id:#?}");
                    if focused {
                        w.is_focused = w.id == id;
                    }
                }
            },
            _ => {},
        }


        let new_state = (1u8..=10u8).map(|idx| {
            state
                .clone()
                .into_iter()
                .find(|wks| wks.idx == idx)
                .unwrap_or_else(|| {
                    NiriWs {
                        id: idx as u64,
                        idx,
                        output: None,
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
        if let Ok(mut socket) = Socket::connect() {
            let _ = socket.send(Request::Action(Action::FocusWorkspace {
                reference: WorkspaceReferenceArg::Id(id),
            }));
        }
    });
}
