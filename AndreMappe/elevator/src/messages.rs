use driver_rust::elevio::elev::{CAB, HALL_DOWN, HALL_UP};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;

use crate::network::world_view::WorldView;
use crate::elevator::elevator::{LocalElevatorStatus, Direction, Behaviour};

pub type NodeId = [u8; 6]; // MAC-sized identity


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ElevatorStatus {
    pub elev_id: NodeId,
    pub behaviour: Behaviour,
    pub floor: u8,
    pub direction: Direction,
    pub cab_calls: HashSet<Call>,
    pub hall_calls: HashSet<Call>,
    pub finished_hall_calls: HashSet<Call>,
    pub known_cab_calls: HashSet<Call>,
    pub is_obstructed: bool,
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
            is_obstructed: false,
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
    pub seq: u64,
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
            "[{}:{}] {} call to floor: {}",
            last_id_byte,
            self.id.seq,
            call_type,
            self.floor,
        )
    }
}

pub struct CallList<'a>(pub&'a HashSet<Call>);

impl<'a> fmt::Display for CallList<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let list = self.0
            .iter()
            .map(|c| format!("{}", c))
            .collect::<Vec<_>>()
            .join(", ");

        write!(f, "{}", list)
    }
}

#[derive(Debug)]
pub enum MsgToElevatorManager {
    /// Recievs a message when the elevator reaches a new floor
    AtFloor(u8),
    /// Revieves a message with the active calls every time there is a change
    ActiveCalls(HashSet<Call>),
    Obstruction(bool),
}

#[derive(Debug)]
pub enum MsgToCallManager {
    /// Recieves the WorldView everytime there is a change
    NewWorldView(WorldView),
    /// Recieves a message when the elevator has finished a call
    ServedCall(Call),
}

#[derive(Debug)]
pub enum MsgToWorldManager {
    AddCall(Call),
    ServedCall(Call),
    NewLocalElevStatus(LocalElevatorStatus),
    NewRemoteElevState(ElevatorStatus),
    AddDisconnectedElevator(NodeId),
    RemoveDisconnectedElevator(NodeId),
}