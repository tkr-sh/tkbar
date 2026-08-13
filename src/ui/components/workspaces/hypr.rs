use {
    super::Ws,
    crate::ui::{CONFIG, components::workspaces::WORKSPACE_COUNT},
    hyprland::{
        data::{Workspace, Workspaces},
        dispatch::{Dispatch, DispatchType, WorkspaceIdentifierWithSpecial},
        event_listener::EventListener,
        prelude::*,
    },
};

pub(super) fn event_loop(tx: &async_channel::Sender<Vec<Ws>>) -> Result<(), String> {
    ipc_loop(tx).map_err(|e| format!("{e:?}"))
}

fn ipc_loop(tx: &async_channel::Sender<Vec<Ws>>) -> hyprland::Result<()> {
    let mut listener = EventListener::new();

    let cli_tx = tx.clone();
    listener.add_workspace_changed_handler(move |_| snapshot(&cli_tx));

    let add_tx = tx.clone();
    listener.add_workspace_added_handler(move |_| snapshot(&add_tx));

    let destroy_tx = tx.clone();
    listener.add_workspace_deleted_handler(move |_| snapshot(&destroy_tx));

    let monitor_tx = tx.clone();
    listener.add_active_monitor_changed_handler(move |_| snapshot(&monitor_tx));

    let open_tx = tx.clone();
    listener.add_window_opened_handler(move |_| snapshot(&open_tx));

    let close_tx = tx.clone();
    listener.add_window_closed_handler(move |_| snapshot(&close_tx));

    snapshot(tx);

    listener.start_listener()
}

fn snapshot(tx: &async_channel::Sender<Vec<Ws>>) {
    let list = match read_workspaces() {
        Ok(list) => list,
        Err(e) => {
            crate::log::warn("workspaces", &format!("hyprland IPC error: {e:?}"));
            return;
        },
    };
    let _ = tx.send_blocking(list);
}

fn read_workspaces() -> hyprland::Result<Vec<Ws>> {
    let focused_id = Workspace::get_active()?.id;

    let mut list: Vec<Ws> = Workspaces::get()?
        .to_vec()
        .into_iter()
        .filter_map(|wks| {
            if wks.id <= 0 {
                return None;
            }
            Some(Ws {
                id: u64::try_from(wks.id).ok()?,
                idx: u8::try_from(wks.id).ok()?,
                label: if CONFIG.security.should_allow_workspace_label {
                    wks.name
                } else {
                    wks.id.to_string()
                },
                is_active: wks.windows > 0,
                is_focused: wks.id == focused_id,
            })
        })
        .collect();

    for idx in 1..=WORKSPACE_COUNT {
        if list.iter().all(|wks| wks.id != u64::from(idx)) {
            list.push(Ws {
                id: u64::from(idx),
                idx,
                label: idx.to_string(),
                is_active: false,
                is_focused: idx == u8::try_from(focused_id).unwrap_or_default(),
            });
        }
    }


    list.sort_by_key(|wks| wks.id);
    Ok(list)
}

pub(super) fn focus_workspace(id: u64) {
    let Ok(id) = i32::try_from(id) else {
        return;
    };
    if let Err(e) = Dispatch::call(DispatchType::Workspace(WorkspaceIdentifierWithSpecial::Id(
        id,
    ))) {
        crate::log::warn("workspaces", &format!("failed to focus workspace: {e:?}"));
    }
}
