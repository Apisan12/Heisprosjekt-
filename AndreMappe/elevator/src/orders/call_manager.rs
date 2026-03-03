use std::collections::{HashMap, HashSet};
use tokio::sync::{mpsc, watch};
use crate::{config::ELEV_NUM_FLOORS, messages::{MsgToFsm, MsgToWorldView, MsgToCallManager, Call, NodeId, ElevState}};
use driver_rust::elevio::{self, elev::{self as e, CAB, Elevator, HALL_DOWN, HALL_UP}};
use crate::orders::assigner;
use crate::network::world_view::WorldView;


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
                    
                        // Turns cab light on, maybe move this to a output task
                        elev.call_button_light(call.floor, call.call_type, true);

                        // The cab call does not need to be verified in the worldview
                        // so it can be added directly to the fsm 
                        let _ = tx_fsm_msg
                                    .send(MsgToFsm::AddCall(call.clone()))
                                    .await;
                    }
                    HALL_DOWN | HALL_UP => { 
                        // Add the call to the worldview
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
            MsgToCallManager::NewWorldView(world) => {

                let active_calls = world.active_calls();
                // Turn on lights for the active hallcalls.
                // Runs the assigner to check if any of the active hall calls is assigned to this node
                // let assigned_hall_calls = run.assigner()
                // for calls in assigned_hall_calls
                //      send(MsgToFsm::AddCall(call.clone())).await;

            }


            MsgToCallManager::FinishedCall(call) => {
                // Turn of light for respective call
                // Add to finished calls
            }

            MsgToCallManager::RestoreCabCalls(calls) => {
                // For calls in calls
                // Turn on light for the cab calls
                // Send cab call to FSM
                // Add cab calls to world view
            }

        }
    }
}