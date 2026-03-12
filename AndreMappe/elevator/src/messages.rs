//! Messaging and shared data structures for the distributed elevator system.
//!
//! This module defines the core types used for communication between the
//! different managers in the system (ElevatorManager, CallManager, and
//! WorldManager). It also contains representations of elevator state,
//! calls, and identifiers used for tracking requests across the network.

use driver_rust::elevio::elev::{CAB, HALL_DOWN, HALL_UP};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;

use crate::network::world_view::WorldView;
use crate::elevator::elevator::{LocalElevatorStatus, Direction, Behaviour};

/// Unique identifier for an elevator.
/// 
/// The ID is based on a MAC-sized array.
pub type ElevatorId = [u8; 6];


/// Represents the current state of an elevator.
/// 
/// This structure is distributed across the network so that all
/// elevators maintain a shared world view of the system.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ElevatorStatus {
    pub elevator_id: ElevatorId,
    pub behaviour: Behaviour,
    pub floor: u8,
    pub direction: Direction,

    /// Local set of cab calls
    pub cab_calls: HashSet<Call>,

    /// Global set of hall calls.
    pub hall_calls: HashSet<Call>,

    /// Global set of served hall calls.
    pub served_hall_calls: HashSet<Call>,

    /// Global set of cab calls.
    pub known_cab_calls: HashSet<Call>,
    
    /// Indicates whether the elevator has faults.
    pub has_faults: bool,
}

impl ElevatorStatus {
    pub fn new(elevator_id: ElevatorId, floor: u8) -> Self {
        Self {
            elevator_id,
            behaviour: Behaviour::Idle,
            floor,
            direction: Direction::Stop,
            cab_calls: HashSet::new(),
            hall_calls: HashSet::new(),
            served_hall_calls: HashSet::new(),
            known_cab_calls: HashSet::new(),
            has_faults: false,
        }
    }
}


#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CallId {
    pub elev_id: ElevatorId,
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
    /// Recievs a message when the elevator reaches a new floor.
    AtFloor(u8),
    /// Revieves a message with the active calls every time there is a change.
    ActiveCalls(HashSet<Call>),
    /// Receives a message when the obstruction state changes.
    Obstruction(bool),
}

#[derive(Debug)]
pub enum MsgToCallManager {
    /// Recieves the WorldView everytime there is a change.
    NewWorldView(WorldView),
}

#[derive(Debug)]
pub enum MsgToWorldManager {
    AddCall(Call),
    ServedCall(Call),
    NewLocalElevatorStatus(LocalElevatorStatus),
    NewRemoteElevState(ElevatorStatus),
    AddDisconnectedElevator(ElevatorId),
    RemoveDisconnectedElevator(ElevatorId),
}