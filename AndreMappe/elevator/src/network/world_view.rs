use driver_rust::elevio::elev::{CAB, HALL_DOWN, HALL_UP};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use tokio::sync::{mpsc, watch};

use crate::assigner::AssignerState;
use crate::messages::{Call, ElevatorStatus, MsgToCallManager, MsgToWorldView, NodeId};
#[derive(Debug, Clone, Serialize)]
pub struct WorldView {
    elevs: HashMap<NodeId, ElevatorStatus>,
}

impl WorldView {
    pub fn new(initial_status: ElevatorStatus) -> Self {
        let mut elevs = HashMap::new();
        elevs.insert(initial_status.elev_id, initial_status);

        Self { elevs }
    }

    /// Creates an iterator for the elevs.
    pub fn elevs(&self) -> impl Iterator<Item = (&NodeId, &ElevatorStatus)> {
        self.elevs.iter()
    }

    /// Merges the hall calls on the other nodes to this node.
    /// This works as an acknoledgment
    pub fn merge_hall_calls(&mut self, elev_id: NodeId) {
        let mut all_hall_calls = HashSet::new();
        let mut all_finished_calls = HashSet::new();

        for elev in self.elevs.values() {
            all_hall_calls.extend(elev.hall_calls.iter().copied());
            all_finished_calls.extend(elev.finished_hall_calls.iter().copied());
        }

        if let Some(elev) = self.elevs.get_mut(&elev_id) {
            for call in all_hall_calls {
                elev.hall_calls.insert(call);
            }
            for call in all_finished_calls {
                elev.finished_hall_calls.insert(call);
            }
        }
    }


    /// Adds the cab calls it has seen on the network to known_cab_calls as an acknowledgment
    pub fn acknowledge_cab_calls(&mut self, elev_id: NodeId) {
        let mut all_cab_calls = HashSet::new();

        for elevator in self.elevs.values() {
            all_cab_calls.extend(elevator.cab_calls.iter().copied());
        }

        if let Some(elevator) = self.elevs.get_mut(&elev_id) {
            elevator.known_cab_calls = all_cab_calls;
        }
    }

    /// checks the worldview for calls that are know on all connected nodes these are active
    /// returns these active calls
    pub fn active_hall_calls(&self) -> HashSet<Call> {
        let mut finished = HashSet::new();

        for elev in self.elevs.values() {
            finished.extend(elev.finished_hall_calls.iter().copied());
        }

        let mut active = if let Some(first) = self.elevs.values().next() {
            first.hall_calls.clone()
        } else {
            return HashSet::new();
        };

        for elev in self.elevs.values() {
            active.retain(|call| elev.hall_calls.contains(call));
        }

        active.retain(|call| !finished.contains(call));

        active
    }

    pub fn active_cab_calls(&self) -> HashSet<Call> {
        todo!();
    }
    /// Gets the mutable local elev state
    pub fn local_elev_mut(&mut self, id: &NodeId) -> &mut ElevatorStatus {
        self.elevs.get_mut(id).expect("Local elevator must exist")
    }

    /// Gets the local elev state read only
    pub fn local_elev(&self, id: &NodeId) -> &ElevatorStatus {
        self.elevs.get(id).expect("Local elevator must exist")
    }

    /// Builds the assigner states to match the input needed for the assigner script
    pub fn assigner_states(&self) -> HashMap<String, AssignerState> {
        let mut states = HashMap::new();

        for (id, elev) in self.elevs() {
            states.insert(format!("id_{id:?}"), AssignerState::from_elev(elev));
        }

        states
    }
}

pub async fn world_manager(
    elev_id: NodeId,
    initial_elev_status: ElevatorStatus,
    mut rx_world_view_msg: mpsc::Receiver<MsgToWorldView>,
    tx_manager_msg: mpsc::Sender<MsgToCallManager>,
    tx_network: watch::Sender<ElevatorStatus>,
) {
    let mut world = WorldView::new(initial_elev_status);

    while let Some(msg) = rx_world_view_msg.recv().await {
        match msg {
            MsgToWorldView::AddCall(call) => {
                let elevator = world.local_elev_mut(&elev_id);
                match call.call_type {
                    CAB => {
                        elevator.cab_calls.insert(call);
                        elevator.known_cab_calls.insert(call);
                    }
                    HALL_DOWN | HALL_UP => {
                        elevator.hall_calls.insert(call);
                    }
                }
                let _ = tx_network.send(elevator.clone());
            }
            MsgToWorldView::FinishedCall(call) => {
                let elevator = world.local_elev_mut(&elev_id);
                match call.call_type {
                    CAB => {
                        elevator.cab_calls.remove(&call);
                    }
                    HALL_DOWN | HALL_UP => {
                        elevator.finished_hall_calls.insert(call);
                    }
                }

                let _ = tx_network.send(elevator.clone());
            }
            MsgToWorldView::UpdateLocalElevStatus(local_elev) => {
                // update behaviour, floor, direction in worldview for this elevators id
                let elev = world.local_elev_mut(&elev_id);
                elev.behaviour = local_elev.behaviour;
                elev.floor = local_elev.floor;
                elev.direction = local_elev.direction;

                let _ = tx_network.send(elev.clone());
            }
            MsgToWorldView::NewRemoteElevState(remote_elev) => {
                // Add the updated elevator state to the world
                world.elevs.insert(remote_elev.elev_id, remote_elev);
                world.merge_hall_calls(elev_id);
                world.acknowledge_cab_calls(elev_id);

                // Sends the new world view to call manager
                let _ = tx_manager_msg
                    .send(MsgToCallManager::NewWorldView(world.clone()))
                    .await;

                let elev = world.local_elev(&elev_id);
                let _ = tx_network.send(elev.clone());
            }
            MsgToWorldView::RemoveDisconnectedElevator(remote_elev_id) => {
                world.elevs.remove(&remote_elev_id);

                let _ = tx_manager_msg
                    .send(MsgToCallManager::NewWorldView(world.clone()))
                    .await;
            }
        }
    }
}
