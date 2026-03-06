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
    // Takes in driver to handle the lights
    driver: Elevator,
    mut rx_manager_msg: mpsc::Receiver<MsgToCallManager>,
    tx_world_view_msg: mpsc::Sender<MsgToWorldView>,
    tx_fsm_msg: mpsc::Sender<MsgToFsm>,
) {
    // Used to store previous active hall calls to determine what lights
    // to turn on or off in the NewWorldView message.
    let mut previous_active_hall_calls: HashSet<Call> = HashSet::new();

    while let Some(msg) = rx_manager_msg.recv().await {
        match msg {
            MsgToCallManager::NewWorldView(world) => {
                let mut all_active_calls: HashSet<Call> = HashSet::new();

                let active_cab_calls = world.active_cab_calls(&elev_id);
                for call in active_cab_calls {
                    driver.call_button_light(call.floor, call.call_type, true);
                    all_active_calls.insert(call);
                }

                let active_hall_calls = world.active_hall_calls();
                // Turn on lights for the newly active calls.
                for call in active_hall_calls.difference(&previous_active_hall_calls) {
                    driver.call_button_light(call.floor, call.call_type, true);
                }
                // Turn off lights for the calls that are no longer active.
                for call in previous_active_hall_calls.difference(&active_hall_calls) {
                    driver.call_button_light(call.floor, call.call_type, false);
                }
                // Update stored active set
                previous_active_hall_calls = active_hall_calls.clone();
                let assigned_calls = assigner::run_assigner(&world, &active_hall_calls, elev_id);
                for call in assigned_calls {
                    all_active_calls.insert(call);
                }

                let _ = tx_fsm_msg
                    .send(MsgToFsm::ActiveCalls(all_active_calls))
                    .await;
            }

            MsgToCallManager::FinishedCall(call) => {
                // Turn of light for respective call
                // Add to finished calls
                match call.call_type {
                    CAB => {
                        driver.call_button_light(call.floor, call.call_type, false);
                        let _ = tx_world_view_msg
                            .send(MsgToWorldView::FinishedCall(call.clone()))
                            .await;
                    }
                    HALL_DOWN | HALL_UP => {
                        let _ = tx_world_view_msg
                            .send(MsgToWorldView::FinishedCall(call.clone()))
                            .await;
                    }
                    other => {
                        eprintln!("Invalid call_type: {other}");
                        continue;
                    }
                }
            }
        }
    }
}
