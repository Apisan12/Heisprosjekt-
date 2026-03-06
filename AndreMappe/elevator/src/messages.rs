use driver_rust::elevio::elev::{CAB, HALL_DOWN, HALL_UP};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;

use crate::network::world_view::WorldView;
use crate::elevator::elevator::LocalElevatorStatus;

pub type NodeId = [u8; 6]; // MAC-sized identity

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Behaviour {
    Idle,
    Moving,
    DoorOpen,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    Up,
    Down,
    Stop,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ElevatorStatus {
    pub elev_id: NodeId,
    pub behaviour: Behaviour,
    pub floor: u8,
    pub direction: Direction,
    pub cab_calls: HashSet<Call>,
    pub hall_calls: HashSet<Call>,
    pub finished_hall_calls: HashSet<Call>,
    pub known_cab_calls: HashSet<Call>,
}

impl ElevatorStatus {
    pub fn new(id: NodeId, floor: u8) -> Self {
        Self {
            elev_id: id,
            behaviour: Behaviour::Idle,
            floor,
            direction: Direction::Stop,
            cab_calls: HashSet::new(),
            hall_calls: HashSet::new(),
            finished_hall_calls: HashSet::new(),
            known_cab_calls: HashSet::new(),
        }
    }
}


// #[derive(Serialize, Deserialize, Clone, Debug)]
// pub struct ElevState {
//     pub behaviour: Behaviour,
//     pub floor: u8,
//     pub direction: Direction,
// }

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CallId {
    pub elev_id: NodeId,
    pub seq: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Call {
    pub id: CallId,
    pub floor: u8,
    pub call_type: u8,
}

impl fmt::Display for Call {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let last_id_byte = self.id.elev_id[5];

        let call_type = match self.call_type {
            CAB => "CAB",
            HALL_UP => "HALL UP",
            HALL_DOWN => "HALL DOWN",
            _ => "UNKNOWN",
        };

        write!(
            f,
            "[{}:{}] floor: {} {}",
            last_id_byte,
            self.id.seq,
            self.floor,
            call_type
        )
    }
}

#[derive(Debug)]
pub enum MsgToFsm {
    AtFloor(u8),
    AddCall(Call),
    DoorTimeout,
}

#[derive(Debug)]
pub enum MsgToCallManager {
    /// New call from the inputs of the elevator
    NewLocalCall(Call),
    /// Sends all the committed hall calls at a set interval
    /// for redundancy from the worldview.
    NewWorldView(WorldView),
    /// Sends the finished call from the FSM.
    FinishedCall(Call),
    /// If the node had unfinished cab calls, they are restored
    /// on initilization with this message.
    _RestoreCabCalls(HashSet<Call>),
}

#[derive(Debug)]
pub enum MsgToWorldView {
    AddCall(Call),
    FinishedCall(Call),
    UpdateLocalElevStatus(LocalElevatorStatus),
    NewRemoteElevState(ElevatorStatus),
    RemoveDisconnectedElevator(NodeId),
}