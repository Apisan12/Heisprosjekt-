use serde::Serialize;
use std::collections::{HashMap, HashSet};
use tokio::sync::{mpsc, watch};

use crate::messages::{Call, ElevState, MsgToCallManager, MsgToWorldView, NodeId};

#[derive(Debug, Clone, Serialize)]
pub struct WorldView {
    elevs: HashMap<NodeId, ElevState>,
}

impl WorldView {
    pub fn new() -> Self {
        Self {
            elevs: HashMap::new(),
        }
    }
    /// Merges the hall calls on the other nodes to this node.
    /// This works as an acknoledgment
    pub fn merge_hall_calls(&mut self, elev_id: NodeId) {
        let mut all_hall_calls = HashSet::new();

        for elev in self.elevs.values() {
            all_hall_calls.extend(elev.hall_calls.iter().copied());
        }

        if let Some(elev) = self.elevs.get_mut(&elev_id) {
            for call in all_hall_calls {
                elev.hall_calls.insert(call);
            }
        }
    }

    /// checks the worldview for calls that are know on all connected nodes these are active
    /// returns these active calls
    pub fn active_calls(&self) -> HashSet<Call> {
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

    /// Gets the mutable local elev state
    pub fn local_elev_mut(&mut self, id: &NodeId) -> &mut ElevState {
        self.elevs.get_mut(id).expect("Local elevator must exist")
    }

    /// Gets the local elev state read only
    pub fn local_elev(&self, id: &NodeId) -> &ElevState {
        self.elevs.get(id).expect("Local elevator must exist")
    }
}

pub async fn world_manager(
    elev_id: NodeId,
    mut rx_world_view_msg: mpsc::Receiver<MsgToWorldView>,
    tx_manager_msg: mpsc::Sender<MsgToCallManager>,
    tx_network: watch::Sender<ElevState>,
) {
    let mut world = WorldView::new();

    while let Some(msg) = rx_world_view_msg.recv().await {
        match msg {
            MsgToWorldView::AddCabCall(cab_call) => {
                let elev = world.local_elev_mut(&elev_id);
                elev.cab_calls.insert(cab_call);

                let _ = tx_network.send(elev.clone());
            }
            MsgToWorldView::RemoveCabCall(cab_call) => {
                let elev = world.local_elev_mut(&elev_id);
                elev.cab_calls.remove(&cab_call);

                let _ = tx_network.send(elev.clone());
            }
            MsgToWorldView::AddHallCall(hall_call) => {
                let elev = world.local_elev_mut(&elev_id);
                elev.hall_calls.insert(hall_call);

                let _ = tx_network.send(elev.clone());
            }
            MsgToWorldView::AddFinishedHallCall(hall_call) => {
                let elev = world.local_elev_mut(&elev_id);
                elev.finished_hall_calls.insert(hall_call);

                let _ = tx_network.send(elev.clone());
            }
            MsgToWorldView::UpdateLocalElevState(this_elev) => {
                // update behaviour, floor, direction in worldview for this elevators id
                let elev = world.local_elev_mut(&elev_id);
                elev.behaviour = this_elev.behaviour;
                elev.floor = this_elev.floor;
                elev.direction = this_elev.direction;

                let _ = tx_network.send(elev.clone());
            }
            MsgToWorldView::NewRemoteElevState(other_elev) => {
                // Add the updated elevator state to the world
                world.elevs.insert(other_elev.id, other_elev);
                world.merge_hall_calls(elev_id);

                // Sends the new world view to call manager
                let _ = tx_manager_msg
                    .send(MsgToCallManager::NewWorldView(world.clone()))
                    .await;

                let elev = world.local_elev(&elev_id);
                let _ = tx_network.send(elev.clone());
            }
        }
    }
}
