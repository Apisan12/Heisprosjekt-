use std::collections::{HashMap, HashSet};
use tokio::sync::{mpsc, watch};
use crate::{config::ELEV_NUM_FLOORS, messages::{Call, LocalState, ManagerMsg, PeerState}};
use driver_rust::elevio::elev::{self as e, CAB};
use crate::orders::assigner;

// ORDER MANAGER
// Tar imot beskjeder på rx_manager kanalen.
// MOTTA
// 1. NewCall
// - Når en knapp blir trykket får inn enn beskjed med typen Call, består av ID til heis som fikk trykket,
//   floor, og call_type 
// - Legges til i worldview -> kjøre assigner -> oppdatere FSM
//
// 2. NetUpdate
// - Får inn PeerState fra en annen node
// - Legges til i wordview -> kjøre assinger -> oppdatere FSM
//
// 3. LocalUpdate
// - Får inn LocalState fra FSM, sende fra FSM hver gang den går i ny state.
// 
// SENDE
// Sende PeerState til de andre nodene på tx_peerstate
// 
// Sende Orders til FSM på tx_fsm
//
// TODO:
// - Lage funksjoner
// - Finne ut hvordan fjerne ferdige ordre.
// - Finne ut hvordan JSON inputen til assigner scriptet skal lages
// - Finne ut hvordan JSON outputen fra assigner blir slik at det kan sendes til FSM

pub struct WorldView {
    pub my_id: u8,
    pub hall_calls: HashSet<Call>,
    pub my_cab_calls: HashSet<Call>, // Usikker om denne trengs siden vi har også cab_calls i PeerState
    pub my_assigned: Vec<[bool; 2]>,
    pub peers: HashMap<u8, PeerState>,
}

pub async fn order_manager(
    my_id: u8,
    mut rx: mpsc::Receiver<ManagerMsg>,
    tx_peerstate: watch::Sender<PeerState>,
    elevator: e::Elevator,
) {
    let mut world = WorldView {
        my_id,
        hall_calls: HashSet::new(),
        my_cab_calls: HashSet::new(),
        my_assigned: vec![[false; 2]; ELEV_NUM_FLOORS as usize],
        peers: HashMap::new(),
    };

    while let Some(msg) = rx.recv().await {

        match msg {
            // 1.
            ManagerMsg::NewCall(call) => {
                match call.call_type {
                    CAB => { world.my_cab_calls.insert(call); }
                    _ => { world.hall_calls.insert(call); }
                }
                world.my_assigned = assigner::run_assigner(&world);
                update_fsm();
            }
            // 2.
            ManagerMsg::NetUpdate(peer) => {

                // if peer.id == world.my_id {
                //     continue;
                // }

                // for call in &peer.hall_calls {
                //     world.hall_calls.insert(call.clone());
                // }

                // world.peers.insert(peer.id,peer);
                // assigner::run_assigner(&world);
                update_fsm();

            }
            // 3.
            ManagerMsg::LocalUpdate(local) => {
                update_peer_state();
            }
        }
        send_peerstate();
        check_all_have_hall_call(&elevator, &world);
    }
}

// Sjekke at alle noder har hall callen før lyset tennes
fn check_all_have_hall_call(elevator: &e::Elevator, world: &WorldView) {
        for call in &world.hall_calls {
            let mut all_have = true;

            // for peer_calls in world.peers.values() {
            //     if !peer_calls.contains(call) {
            //         all_have = false;
            //         break;
            //     }
            // }

            if all_have {
                eprintln!("TENNE LAMPE {:?}", call);
                elevator.call_button_light(call.floor,call.call_type,true);

            }
        }
    }


// Sender ordre til FSM, når den får en ny assigned hall call eller en ny cab call
fn update_fsm() {
    todo!();
}

// Oppdatere PeerState til noden basert på informasjon fra FSM
// (Behaviour, floor, direction)
fn update_peer_state() {
    todo!();
}

// Sende PeerState til de andre nodene
fn send_peerstate() {
    todo!();
}
