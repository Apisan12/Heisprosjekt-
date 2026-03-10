//! Call manager.
//!
//! This module is responsible for managing elevator calls and button lights.
//!
//! The call manager acts as the interface between the distributed
//! world state and the local elevator manager.
//!
//! Responsibilities:
//! - Maintain the correct state of button lights
//! - Determine which calls are active for the local elevator
//! - Run the hall-call assignment algorithm
//! - Notify the world manager when calls have been served
//!
//! The call manager communicates with:
//!
//! - `world_manager` – receives world state updates and reports served calls
//! - `elevator_manager` – sends the set of active calls assigned to this elevator
//! - `driver` – controls the button lights

use crate::{
    calls::assigner,
    messages::{Call, MsgToCallManager, MsgToElevatorManager, MsgToWorldManager, NodeId},
};
use driver_rust::elevio::{
    elev::{Elevator, CAB, HALL_DOWN, HALL_UP},
};
use std::collections::HashSet;
use tokio::sync::mpsc;

/// Asynchronous task responsible for managing elevator calls.
///
/// The call manager receives updates about the global elevator state
/// (`WorldView`) and determines which calls are active for the local
/// elevator. It also ensures that the correct button lights are shown
/// on the elevator panel.
///
/// # Responsibilities
///
/// - Track active hall calls across the system
/// - Track cab calls belonging to this elevator
/// - Run the hall-call assignment algorithm
/// - Update button lights when calls appear or disappear
/// - Notify the world manager when calls have been served
///
/// # Communication
///
/// Receives:
/// - `MsgToCallManager` messages from the world manager and elevator manager
///
/// Sends:
/// - `MsgToElevatorManager::ActiveCalls` to update the local elevator controller
/// - `MsgToWorldManager::ServedCall` when a call has been completed
pub async fn call_manager(
    elev_id: NodeId,
    driver: Elevator,
    mut rx_call_manager: mpsc::Receiver<MsgToCallManager>,
    tx_world_manager: mpsc::Sender<MsgToWorldManager>,
    tx_elevator_manager: mpsc::Sender<MsgToElevatorManager>,
) {
    // Stores the previously known set of active hall calls.
    // Used to determine which hall button lights should be
    // turned on or off when the world view updates.
    let mut previous_active_hall_calls: HashSet<Call> = HashSet::new();

    while let Some(msg) = rx_call_manager.recv().await {
        match msg {

            // A new glogal state snapshot has been received.
            // Update lights and determine which calls this elevator should serve
            MsgToCallManager::NewWorldView(world) => {
                let mut all_active_calls: HashSet<Call> = HashSet::new();

                // Cab calls belong only to this elevator.
                // Ensure their lights are on and add them to the active call set.
                let active_cab_calls = world.active_cab_calls(&elev_id);
                for call in active_cab_calls {
                    driver.call_button_light(call.floor, call.call_type, true);
                    all_active_calls.insert(call);
                }

                let active_hall_calls = world.active_hall_calls();
                // Turn on lights for newly active hall calls.
                for call in active_hall_calls.difference(&previous_active_hall_calls) {
                    driver.call_button_light(call.floor, call.call_type, true);
                }
                // Turn off lights for hall calls that are no longer active.
                for call in previous_active_hall_calls.difference(&active_hall_calls) {
                    driver.call_button_light(call.floor, call.call_type, false);
                }
                // Track the current active hall calls so the next update
                // can determine which lights changed.
                previous_active_hall_calls = active_hall_calls.clone();

                // Run the hall call assignment algorithm to determine which
                // hall calls should be served by this elevator.
                let assigned_calls = assigner::run_assigner(world.clone(), &active_hall_calls, elev_id);
                for call in assigned_calls {
                    all_active_calls.insert(call);
                }

                // Send the complete set of active calls to the elevator manager.
                let _ = tx_elevator_manager
                    .send(MsgToElevatorManager::ActiveCalls(all_active_calls))
                    .await;
            }

            // A call has been served by the elevator.
            // Update button lights and notify the world manager so
            // the global call state can be updated.
            MsgToCallManager::ServedCall(call) => {
                match call.call_type {
                    CAB => {
                        driver.call_button_light(call.floor, call.call_type, false);
                        let _ = tx_world_manager
                            .send(MsgToWorldManager::ServedCall(call.clone()))
                            .await;
                    }
                    HALL_DOWN | HALL_UP => {
                        let _ = tx_world_manager
                            .send(MsgToWorldManager::ServedCall(call.clone()))
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
