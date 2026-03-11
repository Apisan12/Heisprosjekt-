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
//!
//! The call manager communicates with:
//!
//! - `world_manager` – receives world state updates
//! - `elevator_manager` – sends the set of active calls assigned to this elevator
//! - `driver` – controls the button lights

use crate::{
    calls::assigner,
    messages::{Call, MsgToCallManager, MsgToElevatorManager, NodeId},
};
use driver_rust::elevio::elev::Elevator;
use std::collections::HashSet;
use tokio::sync::mpsc;

/// Asynchronous task responsible for managing elevator calls.
///
/// The call manager processes updates from the distributed world state
/// and determines which calls are active for the local elevator. It also
/// ensures that the correct button lights are shown on the elevator panel.
///
/// Receives `MsgToCallManager` messages containing updated world views,
/// updates cab and hall button lights, runs the hall-call assignment
/// algorithm, and sends the resulting set of active calls to the
/// `elevator_manager`.
pub async fn call_manager(
    elev_id: NodeId,
    driver: Elevator,
    mut rx_call_manager: mpsc::Receiver<MsgToCallManager>,
    tx_elevator_manager: mpsc::Sender<MsgToElevatorManager>,
) {
    // Stores the previously known set of active hall calls.
    // Used to determine which hall button lights should be
    // turned on or off when the world view updates.
    let mut previous_active_hall_calls: HashSet<Call> = HashSet::new();
    let mut previous_active_cab_calls: HashSet<Call> = HashSet::new();

    while let Some(msg) = rx_call_manager.recv().await {
        match msg {
            // A new global state snapshot has been received.
            // Update lights and determine which calls this elevator should serve
            MsgToCallManager::NewWorldView(world) => {
                let mut all_active_calls: HashSet<Call> = HashSet::new();

                // Cab calls belong only to this elevator.
                // Ensure their lights are on and add them to the active call set.
                let active_cab_calls = world.active_cab_calls(&elev_id);

                // Turn on newly active cab calls
                for call in active_cab_calls.difference(&previous_active_cab_calls) {
                    driver.call_button_light(call.floor, call.call_type, true);
                }

                // Turn off cleared cab calls
                for call in previous_active_cab_calls.difference(&active_cab_calls) {
                    driver.call_button_light(call.floor, call.call_type, false);
                }
                previous_active_cab_calls = active_cab_calls.clone();

                for call in active_cab_calls {
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
                let assigned_calls =
                    assigner::run_assigner(world.clone(), &active_hall_calls, elev_id);
                for call in assigned_calls {
                    all_active_calls.insert(call);
                }

                // Send the complete set of active calls to the elevator manager.
                let _ = tx_elevator_manager
                    .send(MsgToElevatorManager::ActiveCalls(all_active_calls))
                    .await;
            },
        }
    }
}
