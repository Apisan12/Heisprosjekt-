
use crate::{
    orders::assigner,
    messages::{Call, MsgToCallManager, MsgToFsm, MsgToWorldView, NodeId},
};
use driver_rust::elevio::{
    elev::{Elevator, CAB, HALL_DOWN, HALL_UP},
};
use std::collections::HashSet;
use tokio::sync::mpsc;

pub async fn call_manager(
    elev_id: NodeId,
    // Takes in elevator to handle the lights, maybe make another task to handle outputs?
    // So we dont need to have elevator in the call_manager scope.
    elev: Elevator,
    mut rx_manager_msg: mpsc::Receiver<MsgToCallManager>,
    tx_world_view_msg: mpsc::Sender<MsgToWorldView>,
    tx_fsm_msg: mpsc::Sender<MsgToFsm>,
) {
    // Used to store previous active hall calls to determine what lights
    // to turn on or off in the NewWorldView message.
    let mut previous_active: HashSet<Call> = HashSet::new();

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
                        let _ = tx_fsm_msg.send(MsgToFsm::AddCall(call.clone())).await;
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

                // If in finished and not in active turn off light

                // Turn on lights for the newly active calls.
                for call in active_calls.difference(&previous_active) {
                    elev.call_button_light(call.floor, call.call_type, true);
                }

                // Turn off lights for the calls that are no longer active.
                for call in previous_active.difference(&active_calls) {
                    elev.call_button_light(call.floor, call.call_type, false);
                }

                // Update stored active set
                previous_active = active_calls.clone();

                let assigned_calls = assigner::run_assigner(&world, &active_calls, elev_id);
                for call in assigned_calls {
                    let _ = tx_fsm_msg.send(MsgToFsm::AddCall(call.clone())).await;
                }
            }

            MsgToCallManager::FinishedCall(call) => {
                // Turn of light for respective call
                // Add to finished calls
                match call.call_type {
                    CAB => {
                        elev.call_button_light(call.floor, call.call_type, false);
                        let _ = tx_world_view_msg
                            .send(MsgToWorldView::RemoveCabCall(call.clone()))
                            .await;
                    }
                    HALL_DOWN | HALL_UP => {
                        let _ = tx_world_view_msg
                            .send(MsgToWorldView::AddFinishedHallCall(call.clone()))
                            .await;
                    }
                    other => {
                        eprintln!("Invalid call_type: {other}");
                        continue;
                    }
                }
            }

            MsgToCallManager::RestoreCabCalls(calls) => {
                for call in calls {
                    elev.call_button_light(call.floor, call.call_type, true);
                }
                // For calls in calls
                // Turn on light for the cab calls
                // Send cab call to FSM
                // Add cab calls to world view
            }
        }
    }
}
