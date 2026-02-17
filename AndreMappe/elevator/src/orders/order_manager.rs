use std::collections::{HashMap, HashSet};
use tokio::sync::{mpsc, watch};
use crate::messages::{Call, PeerState, LocalState, ManagerMsg};
use driver_rust::elevio::elev::{self as e, CAB};

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

struct WorldView {
    my_id: u8,
    hall_calls: HashSet<Call>,
    my_cab_calls: HashSet<Call>,
    my_assigned: HashSet<Call>,
    peers: HashMap<u8, PeerState>,
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
        my_assigned: HashSet::new(),
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
                run_assigner();
                update_fsm();
            }
            // 2.
            ManagerMsg::NetUpdate(peer) => {

                if peer.id == world.my_id {
                    continue;
                }

                for call in &peer.hall_calls {
                    world.hall_calls.insert(call.clone());
                }

                world.peers.insert(peer.id,peer);
                run_assigner();
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

// Kjøre assigner skriptet som deretter legger til ordren i my_assigned til noden som skal ta ordren
fn run_assigner() {
    todo!();
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