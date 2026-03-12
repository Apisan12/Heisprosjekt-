//! World state management for the distributed elevator system.
//!
//! This module maintains the shared `WorldView`, which represents the
//! known status of all elevators in the network. 
//!
//! The `world_manager` task is responsible for updating the `WorldView`
//! based on incoming messages and propagating updates to other components
//! and the network. It receives events such as new calls, served calls,
//! elevator state updates, and connectivity changes, and ensures the
//! distributed state remains consistent across elevators.
//!
//! The `WorldView` also provides helper methods for:
//! - determining active hall and cab calls
//! - merging hall calls received from other elevators
//! - acknowledging cab calls
//! - cleaning up calls that have been served
//! - building input state for the hall request assigner.

use driver_rust::elevio::elev::{CAB, HALL_DOWN, HALL_UP};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use tokio::sync::{mpsc, watch};

use crate::assigner::AssignerState;
use crate::messages::{
    Call, ElevatorStatus, MsgToCallManager, MsgToWorldManager, ElevatorId,
};

/// Shared view of all elevators in the distributed system.
/// 
/// `WorldView` stores the latest known status of every elevator and
/// tracks which elevators are currently disconnected. The structure
/// is updated by the `world_manager` when new statuses are received
/// from the local elevator or remote elevators on the network.
#[derive(Debug, Clone, Serialize)]
pub struct WorldView {
    elevators: HashMap<ElevatorId, ElevatorStatus>,
    disconnected_elevators: HashSet<ElevatorId>,
}

impl WorldView {

    /// Creates a new `WorldView` containing the local elevator.
    pub fn new(initial_status: ElevatorStatus) -> Self {
        let mut elevators = HashMap::new();
        elevators.insert(initial_status.elevator_id, initial_status);

        Self {
            elevators,
            disconnected_elevators: HashSet::new(),
        }
    }

    /// Returns an iterator over all known elevators.
    pub fn elevators(&self) -> impl Iterator<Item = (&ElevatorId, &ElevatorStatus)> {
        self.elevators.iter()
    }

    /// Returns an iterator over all currently connected elevators.
    pub fn connected_elevators(&self) -> impl Iterator<Item = (&ElevatorId, &ElevatorStatus)> {
        self.elevators
            .iter()
            .filter(move |(id, _)| !self.disconnected_elevators.contains(*id))
    }

    /// Returns a mutable reference to the local elevator status.
    pub fn local_elevator_mut(&mut self, elevator_id: &ElevatorId) -> &mut ElevatorStatus {
        self.elevators
            .get_mut(elevator_id)
            .expect("Local elevator must exist")
    }

    /// Returns a immutable reference to the local elevator status.
    pub fn local_elev(&self, elevator_id: &ElevatorId) -> &ElevatorStatus {
        self.elevators.get(elevator_id).expect("Local elevator must exist")
    }

    /// Merges the hall calls received from other elevators.
    /// 
    /// Hall calls and served hall calls from all connected elevators
    /// are combined and applied to the local elevator status. This
    /// synchronizes the global sets of hall calls across the elevators and ensures
    /// the hall call information propagates across the netwrok.
    pub fn merge_hall_calls(&mut self, elevator_id: &ElevatorId) {
        let mut all_hall_calls = HashSet::new();
        let mut all_served_hall_calls = HashSet::new();

        for (_, elev) in self.connected_elevators() {
            all_hall_calls.extend(elev.hall_calls.iter().copied());
            all_served_hall_calls.extend(elev.served_hall_calls.iter().copied());
        }

        if let Some(elevator) = self.elevators.get_mut(elevator_id) {
            for call in &all_hall_calls {
                // Inserts all hall calls that are not served by any elevator.
                if !all_served_hall_calls.contains(call) {
                    elevator.hall_calls.insert(*call);
                }
            }
            for call in &all_served_hall_calls {
                // Only merge served calls that still correspond to an active hall call
                // To avoid adding back a served call that has been cleaned up.
                if all_hall_calls.contains(call) {
                    elevator.served_hall_calls.insert(*call);
                }
            }
        }
    }

    /// Updates the local elevator's `known_cab_calls`.
    /// 
    /// All cab calls observed in the system are stored in the local elevator status.
    /// This allows other elevators to confirm that their cab call is backed up on the network.
    pub fn acknowledge_cab_calls(&mut self, elev_id: &ElevatorId) {
        let mut all_cab_calls = HashSet::new();

        for (_, elevator) in self.elevators() {
            all_cab_calls.extend(elevator.cab_calls.iter().copied());
        }

        if let Some(elevator) = self.elevators.get_mut(elev_id) {
            elevator.known_cab_calls = all_cab_calls;
        }
    }

    /// Returns the set of active hall calls.
    /// 
    /// A hall call is considered active when:
    /// - it exists on all connected elevators
    /// - it has not been marked as served by any elevator
    /// 
    /// This makes sure all the elevators agree that the hall call is active.
    pub fn active_hall_calls(&self) -> HashSet<Call> {
        let mut served = HashSet::new();

        for (_, elevator) in self.connected_elevators() {
            served.extend(elevator.served_hall_calls.iter().copied());
        }

        // Computes the intersection of hall calls across all connected elevators.
        let mut active = if let Some((_, first)) = self.connected_elevators().next() {
            first.hall_calls.clone()
        } else {
            return HashSet::new();
        };
        for (_, elevator) in self.connected_elevators() {
            active.retain(|call| elevator.hall_calls.contains(call));
        }

        // Remove hall calls that have already been marked as served.
        active.retain(|call| !served.contains(call));

        active
    }

    /// Returns the set of active cab calls for a specific elevator.
    /// 
    /// Cab calls are considered active when they are known by the
    /// other elevators in the network, indicating that the cab call
    /// has been successfully backed up.
    /// 
    /// If the elevator is the only one in the network, all cab calls
    /// are considered active immediately.
    pub fn active_cab_calls(&self, elevator_id: &ElevatorId) -> HashSet<Call> {
        let elevator = self
            .elevators
            .get(elevator_id)
            .expect("Elevator not in worldview");
        let mut active: HashSet<Call> = elevator.cab_calls.iter().copied().collect();

        // If the elevator is alone in the worldview
        if self.connected_elevators().all(|(id, _)| id == elevator_id) {
            return active;
        }

        // Only keep cab calls known by other elevators as active
        active.retain(|call| {
            self.connected_elevators()
                .filter(|(id, _)| *id != elevator_id)
                .all(|(_, other)| other.known_cab_calls.contains(call))
        });

        active
    }

    /// Returns the elevator state map used by the hall request assigner.
    /// 
    /// Each connected elevator is converted into an `AssignerState`
    /// and stored using the identifier format expected by the external assigner script.
    pub fn assigner_states(&self) -> HashMap<String, AssignerState> {
        let mut states = HashMap::new();

        for (elevator_id, elevator) in self.connected_elevators() {
            states.insert(format!("id_{elevator_id:?}"), AssignerState::from_elevator(&self, elevator));
        }

        states
    }

    /// Cleans up hall calls that have been fully served.
    ///
    /// The removal process happens in two stages:
    /// 1. Hall calls marked as served by all elevators are removed
    ///    from the local elevator's hall call list.
    /// 2. Served call markers are removed once no elevator has
    ///    the call in their hall call list.
    /// 
    /// This two-phase process ensures safe propagation of call
    /// completion across the distributed system.
    pub fn cleanup_hall_calls(&mut self, elevator_id: &ElevatorId) {
        let mut served_by_all: HashSet<Call> = self
            .connected_elevators()
            .flat_map(|(_, elevator)| elevator.served_hall_calls.iter().copied())
            .collect();

        served_by_all.retain(|call| {
            self.connected_elevators()
                .any(|(_, elevator)| elevator.served_hall_calls.contains(call))
        });

        // Remove the hall call only from the local elevator
        if let Some(local) = self.elevators.get_mut(elevator_id) {
            for call in &served_by_all {
                local.hall_calls.remove(call);
            }
        }

        let removable_served_markers: HashSet<Call> = served_by_all
            .into_iter()
            .filter(|call| {
                !self
                    .connected_elevators()
                    .any(|(_, elevator)| elevator.hall_calls.contains(call))
            })
            .collect();

        for elevator in self.elevators.values_mut() {
            for call in &removable_served_markers {
                elevator.served_hall_calls.remove(call);
            }
        }
    }

    /// Removes elevators marked as has_faults from the `WorldView`.
    /// 
    /// This is used to remove faulty elevators from the `Worldview` 
    /// snapshot the assigner gets, ensuring that calls are not assigned to
    /// elevators that currently report faults.
    pub fn remove_faulty_elevators(&mut self) {
        self.elevators.retain(|_,elevator| !elevator.has_faults);
    }
}

/// Asynchronous task responsible for maintaining the shared `WorldView`.
/// 
/// The `world_manager` receives updates from other system component
/// through `MsgToWorldManager` messages and applies them to the
/// distributed world state. This includes events such as new calls,
/// served calls, elevator status updates, and connectivity changes.
/// 
/// After updating the `WorldView`, the task:
/// - Broadcasts the local elevator status to the network
/// - Sends the updated `WorldView` to the `call_manager`
pub async fn world_manager(
    elevator_id: ElevatorId,
    initial_elevator_status: ElevatorStatus,
    mut rx_world_manager: mpsc::Receiver<MsgToWorldManager>,
    tx_call_manager: mpsc::Sender<MsgToCallManager>,
    tx_network: watch::Sender<ElevatorStatus>,
) {
    let mut world = WorldView::new(initial_elevator_status);

    while let Some(msg) = rx_world_manager.recv().await {
        match msg {

            // Triggers when a new call is detected locally.
            MsgToWorldManager::AddCall(call) => {
                {
                    println!("Call recieved in WorldView: {}", call);
                    let elevator = world.local_elevator_mut(&elevator_id);
                    match call.call_type {
                        CAB => {
                            elevator.cab_calls.insert(call);
                            elevator.known_cab_calls.insert(call);
                        }
                        HALL_DOWN | HALL_UP => {
                            elevator.hall_calls.insert(call);
                        }
                        _ => {}
                    }
                    println!("Transmitting to network:\n{}", call);
                    let _ = tx_network.send(elevator.clone());
                }
                let _ = tx_call_manager
                    .send(MsgToCallManager::NewWorldView(world.clone()))
                    .await;
            }

            // Triggers when the local elevator has served a call.
            MsgToWorldManager::ServedCall(call) => {
                {
                    let elevator = world.local_elevator_mut(&elevator_id);
                    match call.call_type {
                        CAB => {
                            elevator.cab_calls.remove(&call);
                        }
                        HALL_DOWN | HALL_UP => {
                            elevator.served_hall_calls.insert(call);
                            println!("Call served: {}", call)
                        }
                        _ => {}
                    }
                    let _ = tx_network.send(elevator.clone());
                }
                let _ = tx_call_manager
                    .send(MsgToCallManager::NewWorldView(world.clone()))
                    .await;
            }

            // Triggers when the local elevator has changed status.
            MsgToWorldManager::NewLocalElevatorStatus(local_elev) => {
                // update behaviour, floor, direction in worldview for this elevators id
                let elev = world.local_elevator_mut(&elevator_id);
                elev.behaviour = local_elev.behaviour;
                elev.floor = local_elev.floor;
                elev.direction = local_elev.direction;
                elev.has_faults = local_elev.has_faults;

                let _ = tx_network.send(elev.clone());
            }

            // Triggers when a elevator status is received from a remote elevator.
            MsgToWorldManager::NewRemoteElevatorStatus(remote_elev) => {
                world.elevators.insert(remote_elev.elevator_id, remote_elev);
                world.merge_hall_calls(&elevator_id);
                world.cleanup_hall_calls(&elevator_id);
                world.acknowledge_cab_calls(&elevator_id);

                let elev = world.local_elev(&elevator_id);
                let _ = tx_network.send(elev.clone());

                let _ = tx_call_manager
                    .send(MsgToCallManager::NewWorldView(world.clone()))
                    .await;
            }

            // Triggers when a elevator is disconnected from the network.
            MsgToWorldManager::AddDisconnectedElevator(remote_elev_id) => {
                world.disconnected_elevators.insert(remote_elev_id);

                let _ = tx_call_manager
                    .send(MsgToCallManager::NewWorldView(world.clone()))
                    .await;
            }

            // Triggers when a disconnected elevator reconnects.
            MsgToWorldManager::RemoveDisconnectedElevator(remote_elev_id) => {
                world.disconnected_elevators.remove(&remote_elev_id);

                let _ = tx_call_manager
                    .send(MsgToCallManager::NewWorldView(world.clone()))
                    .await;
            }
        }
    }
}
