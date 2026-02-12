use std::collections::{HashMap, HashSet};
use tokio::sync::{mpsc, watch};
use crate::domain::messages::{Call, NetState};
// use driver_rust::elevio::poll;

pub enum LogicMsg {
    LocalButton(Call),
    NetUpdate(NetState),
}

struct WorldView {
    my_id: u8,
    my_calls: HashSet<Call>,
    peers: HashMap<u8, HashSet<Call>>,
}

pub async fn logic_loop(
    my_id: u8,
    mut rx: mpsc::Receiver<LogicMsg>,
    tx_snapshot: watch::Sender<NetState>,
) {
    let mut world = WorldView {
        my_id,
        my_calls: HashSet::new(),
        peers: HashMap::new(),
    };

    while let Some(msg) = rx.recv().await {

        match msg {
            LogicMsg::LocalButton(call) => {
                world.my_calls.insert(call);
            }

            LogicMsg::NetUpdate(state) => {
                world.peers.insert(state.id, state.calls);
            }
        }

        // send snapshot til network sender
        let snapshot = NetState {
            id: world.my_id,
            calls: world.my_calls.clone(),
        };

        tx_snapshot.send(snapshot).ok();

        check_all_seen(&world);
    }
}

fn check_all_seen(world: &WorldView) {

    for call in &world.my_calls {

        let mut all_have = true;

        for peer_calls in world.peers.values() {
            if !peer_calls.contains(call) {
                all_have = false;
                break;
            }
        }

        if all_have {
            println!("TENNE LAMPE {:?}", call);
        }
    }
}
