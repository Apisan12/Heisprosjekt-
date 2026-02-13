use std::collections::{HashMap, HashSet};
use driver_rust::elevio::elev;
use tokio::sync::{mpsc, watch};
use crate::domain::messages::{Call, NetState};
use driver_rust::elevio::elev as e;

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
    elevator: e::Elevator,
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
                // println!("NetUpdate received: state.id={}, my_id={}", state.id, world.my_id);

                if state.id == world.my_id {
                    continue;
                }

                for call in &state.calls {
                    world.my_calls.insert(call.clone());
                }

                world.peers.insert(state.id, state.calls);

            }
        }

        // send snapshot til network sender
        let snapshot = NetState {
            id: world.my_id,
            calls: world.my_calls.clone(),
        };

        tx_snapshot.send(snapshot).ok();
        // println!("my_calls len = {:?}", world.my_calls);
        // println!("my_peers len = {:?}", world.peers);

        check_all_seen(&elevator, &world);
    }
}

fn check_all_seen(elevator: &e::Elevator, world: &WorldView) {
    // println!("Sjekker alle");
    for call in &world.my_calls {
        // println!("Call: {:?}", call);
        let mut all_have = true;

        for peer_calls in world.peers.values() {
            // println!("World peers: {:?}", world.peers);
            if !peer_calls.contains(call) {
                all_have = false;
                break;
            }
        }

        if all_have {
            eprintln!("TENNE LAMPE {:?}", call);
            elevator.call_button_light(call.floor,call.call,true);

        }
    }
}
