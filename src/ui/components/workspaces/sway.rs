use {
    super::Ws,
    crate::conf::CONFIG,
    std::collections::HashMap,
    swayipc::{Connection, Event, EventType, Node, NodeType},
};

pub(super) fn event_loop(tx: &async_channel::Sender<Vec<Ws>>) -> Result<(), String> {
    ipc_loop(tx).map_err(|e| format!("{e}"))
}

fn ipc_loop(tx: &async_channel::Sender<Vec<Ws>>) -> swayipc::Fallible<()> {
    let connection = Connection::new()?;
    let stream = connection.subscribe([EventType::Workspace, EventType::Window])?;

    // `subscribe` consumes the connection, so every snapshot must open a fresh one.
    snapshot(tx);

    for event in stream {
        match event? {
            Event::Workspace(_) | Event::Window(_) => snapshot(tx),
            Event::Shutdown(_) => return Ok(()),
            _ => {},
        }
    }

    Ok(())
}

fn snapshot(tx: &async_channel::Sender<Vec<Ws>>) {
    let list = match read_workspaces() {
        Ok(list) => list,
        Err(e) => {
            crate::log::warn("workspaces", &format!("sway IPC error: {e}"));
            return;
        },
    };
    let _ = tx.send_blocking(list);
}

fn read_workspaces() -> swayipc::Fallible<Vec<Ws>> {
    let mut conn = Connection::new()?;
    let workspaces = conn.get_workspaces()?;
    let counts = tree_window_counts(&conn.get_tree()?);

    let mut list: Vec<Ws> = workspaces
        .into_iter()
        .map(|ws| {
            Ws {
                id: u64::try_from(ws.id).unwrap_or(0),
                idx: u8::try_from(ws.num).unwrap_or(0),
                label: if CONFIG.security.should_allow_workspace_label {
                    ws.name
                } else {
                    ws.num.to_string()
                },
                is_active: counts.get(&ws.id).is_some_and(|count| *count > 0),
                is_focused: ws.focused,
            }
        })
        .collect();

    if CONFIG.behaviour.should_show_empty_workspace {
        for idx in 1..=CONFIG.behaviour.workspace_count {
            if list.iter().all(|wks| wks.idx != idx) {
                list.push(Ws {
                    id: u64::from(idx),
                    idx,
                    label: idx.to_string(),
                    is_active: false,
                    is_focused: false,
                });
            }
        }
    }

    list.sort_unstable_by_key(|wks| wks.idx);
    Ok(list)
}

/// Windows per workspace, keyed by the workspace node id. `get_workspaces`
/// does not expose window counts, so they are derived from the layout tree: a
/// leaf container (`NodeType::Con` with no children) is a window.
fn tree_window_counts(root: &Node) -> HashMap<i64, u32> {
    let mut map = HashMap::new();
    collect_workspaces(root, &mut map);
    map
}

fn collect_workspaces(node: &Node, map: &mut HashMap<i64, u32>) {
    if node.node_type == NodeType::Workspace {
        map.insert(node.id, window_count(node));
    }
    for child in node.nodes.iter().chain(node.floating_nodes.iter()) {
        collect_workspaces(child, map);
    }
}

fn window_count(node: &Node) -> u32 {
    if node.node_type == NodeType::Con && node.nodes.is_empty() && node.floating_nodes.is_empty() {
        return 1;
    }
    node.nodes
        .iter()
        .chain(node.floating_nodes.iter())
        .map(window_count)
        .sum()
}

pub(super) fn focus_workspace(id: u64) {
    let Ok(id) = i64::try_from(id) else {
        return;
    };
    let Ok(mut conn) = Connection::new() else {
        return;
    };
    let workspaces = match conn.get_workspaces() {
        Ok(list) => list,
        Err(e) => {
            crate::log::warn("workspaces", &format!("sway IPC error: {e}"));
            return;
        },
    };

    let cmd = match workspaces.iter().find(|ws| ws.id == id) {
        Some(ws) => format!("workspace number {}", ws.num),
        None => format!("workspace number {id}"),
    };

    if let Err(e) = conn.run_command(&cmd) {
        crate::log::warn("workspaces", &format!("failed to focus workspace: {e}"));
    }
}
