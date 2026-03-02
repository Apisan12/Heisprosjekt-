use std::collections::{HashMap, HashSet};
use tokio::sync::{mpsc, watch};
use crate::{config::ELEV_NUM_FLOORS, messages::{MsgToFsm, MsgToWorldView, MsgToCallManager, Call, NodeId, PeerState}, orders::world_view::WorldView};
use driver_rust::elevio::{self, elev::{self as e, CAB, Elevator, HALL_DOWN, HALL_UP}};
use crate::orders::{assigner, world_view};


pub async fn call_manager(
    my_id: NodeId,
    // Takes in elevator to handle the lights, maybe make another task to handle outputs?
    // So we dont need to have elevator in the call_manager scope.
    elev: Elevator,
    mut rx_manager_msg: mpsc::Receiver<MsgToCallManager>,
    tx_world_view_msg: mpsc::Sender<MsgToWorldView>,
    tx_fsm_msg: mpsc::Sender<MsgToFsm>,
) { 

    while let Some(msg) = rx_manager_msg.recv().await {

        match msg {
            MsgToCallManager::NewLocalCall(call) => {
                match call.call_type {
                    CAB => { 
                        // Add the call to the worldview
                        let _ = tx_world_view_msg
                                    .send(MsgToWorldView::AddCabCall(call.clone()))
                                    .await;
                        // The cab call does not need to be verified in the worldview
                        // so it can be added directly to the fsm 

                        // Turns cab light on, maybe move this to a output task
                        elev.call_button_light(call.floor, call.call_type, true);

                        let _ = tx_fsm_msg
                                    .send(MsgToFsm::AddCall(call.clone()))
                                    .await;
                    }
                    HALL_DOWN | HALL_UP => { 
                        // Add the call to the worldview
                        // This does not need to be sent to the FSM since it needs to be
                        let _ = tx_world_view_msg
                                    .send(MsgToWorldView::AddHallCall(call.clone()))
                                    .await;
                    }
                    // ##TODO: Add some error handling here if the call type is not correct?
                    other => {
                        eprintln!("Invalid call_type: {other}");
                        continue;
                    }
                }
            }
            MsgToCallManager::ActiveHallCalls(calls) => {
                // Turn on lights for the active hallcalls.
                // Runs the assigner to check if any of the active hall calls is assigned to this node
                // let assigned_hall_calls = run.assigner()
                // for calls in assigned_hall_calls
                //      send(MsgToFsm::AddCall(call.clone())).await;

            }


            MsgToCallManager::FinishedCall(call) => {
                todo!()
            }

        }
    }
}


// Sender ordre til FSM, når den får en ny assigned hall call eller en ny cab call
async fn send_orders_to_fsm(
    tx_fsm_msg: mpsc::Sender<MsgToFsm>, 
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
    
    tx_fsm_msg.send(MsgToFsm::OrdersUpdated(calls)).await.ok();
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
