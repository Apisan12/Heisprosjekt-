use std::collections::{HashMap, HashSet};
use tokio::sync::{mpsc, watch};
use crate::{config::ELEV_NUM_FLOORS, messages::{FsmMsg, LocalState, ManagerMsg, Call, NodeId, PeerState}, orders::world_view::WorldView};
use driver_rust::elevio::elev::{self as e, CAB, HALL_DOWN, HALL_UP};
use crate::orders::{assigner, world_view};

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


pub async fn order_manager(
    my_id: NodeId,
    initial_peer_state: PeerState,
    mut rx_manager_msg: mpsc::Receiver<ManagerMsg>,
    tx_peer_state: watch::Sender<PeerState>,
    tx_fsm_msg: mpsc::Sender<FsmMsg>,
) {
    let mut world = WorldView::new();

    let mut my_cab_calls = vec![false; ELEV_NUM_FLOORS as usize];
    let mut my_assigned_hall_calls = vec![[false; 2]; ELEV_NUM_FLOORS as usize];

    while let Some(msg) = rx_manager_msg.recv().await {

        match msg {
            // 1.
            ManagerMsg::NewCall(call) => {
                match call.call_type {
                    CAB => { 
                        my_cab_calls[call.floor as usize] = true;
                    }
                    HALL_DOWN | HALL_UP => { 
                        world.pending_calls.insert(call); 
                    }
                }

                my_assigned_hall_calls = assigner::run_assigner(&world);

                update_fsm(tx_fsm_msg.clone(), &my_cab_calls, &my_assigned_hall_calls);
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
                update_fsm(tx_fsm_msg.clone(), &my_cab_calls, &my_assigned_hall_calls);

            }
            // 3.
            ManagerMsg::LocalUpdate(local) => {
                update_my_peer_state(&mut world, local, &my_id);
            }

            ManagerMsg::OrderFinished(call) => {
                todo!()
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
async fn update_fsm(
    tx_fsm_msg: mpsc::Sender<FsmMsg>, 
    cab_calls: &Vec<bool>, 
    hall_calls: &Vec<[bool; 2]>
) {
    let mut calls: Vec<[bool; 3]> = Vec::new();
    
    for floor in 0..ELEV_NUM_FLOORS as usize {
        calls.push([
            hall_calls[floor][0],
            hall_calls[floor][1],
            cab_calls[floor],
        ]);
    }
    
    tx_fsm_msg.send(FsmMsg::OrdersUpdated(calls)).await.ok();
}

// Oppdatere PeerState til noden basert på informasjon fra FSM
// (Behaviour, floor, direction)
fn update_my_peer_state(
    world: &mut WorldView, 
    my_state: LocalState, 
    my_id: &NodeId
) {
    if let Some(peer) = world.peers.get_mut(my_id) {
        peer.behaviour = my_state.behaviour;
        peer.floor = my_state.floor;
        peer.direction = my_state.direction;
    }
}

// Oppdaterer Peers med ny PeerState
fn broadcast_peer_state(tx_peer_state: watch::Sender<PeerState>) {
    todo!();
}
