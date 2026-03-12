//! Messaging and shared data structures for the distributed elevator system.
//!
//! This module defines the core types used for communication between the
//! different managers in the system (ElevatorManager, CallManager, and
//! WorldManager). It also contains representations of elevator status,
//! calls, and identifiers used in the messages.

use driver_rust::elevio::elev::{CAB, HALL_DOWN, HALL_UP};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;

use crate::network::world_view::WorldView;
use crate::elevator::elevator::{Direction, Behaviour};

/// Unique identifier for an elevator.
/// 
/// The ID is based on a MAC-sized array.
pub type ElevatorId = [u8; 6];

/// Represents the current status of an elevator.
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

    /// Remote cab calls this elevator knows about.
    pub known_cab_calls: HashSet<Call>,
    
    /// Indicates whether the elevator currently has detected faults.
    pub has_faults: bool,
}

impl ElevatorStatus {

    /// Creates a new instance of the ElevatorStatus struct
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

/// Status information about the local elevator.
///
/// This struct is sent to the `world_manager` whenever the
/// local elevator status changes. 
#[derive(Debug, Clone)]
pub struct LocalElevatorStatus {
    pub floor: u8,
    pub direction: Direction,
    pub behaviour: Behaviour,
    /// Indicates whether the elevator currently has detected faults.
    pub has_faults: bool,
}

impl LocalElevatorStatus {
    /// Creates a new elevator status message.
    pub fn new(floor: u8, direction: Direction, behaviour: Behaviour, has_faults: bool) -> Self {
        Self {
            floor,
            direction,
            behaviour,
            has_faults,
        }
    }
}

/// Identifier for a call.
/// 
/// Each call is uniquely identified by:
/// - the elevator that generated the call
/// - a monotonically increasing sequence number
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CallId {
    pub elevator_id: ElevatorId,
    pub seq: u64,
}

/// Representation of a call.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Call {
    pub call_id: CallId,
    pub floor: u8,
    pub call_type: u8,
}

/// Display function for printing a call.
/// 
/// Usefull for logging and debugging.
impl fmt::Display for Call {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let last_id_byte = self.call_id.elevator_id[5];

        let call_type = match self.call_type {
            CAB => "CAB",
            HALL_UP => "HALL UP",
            HALL_DOWN => "HALL DOWN",
            _ => "UNKNOWN",
        };

        write!(
            f,
            "[{:?}:{}] {} call to floor: {}",
            last_id_byte,
            self.call_id.seq,
            call_type,
            self.floor,
        )
    }
}

/// Helper wrapper used for printing a list of calls.
/// 
/// Useful for logging and debugging
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

/// Messages sent to the `elevator_manager`.
#[derive(Debug)]
pub enum MsgToElevatorManager {
    /// Sent when the elevator arrives at a floor.
    AtFloor(u8),
    /// Sent whenever the set of active calls changes.
    ActiveCalls(HashSet<Call>),
    /// Sent when the obstruction switch changes state.
    Obstruction(bool),
}

/// Messages sent to the `call_manager`.
#[derive(Debug)]
pub enum MsgToCallManager {
    /// Sent whenever the `WorldView` changes.
    NewWorldView(WorldView),
}

/// Messages sent to the `world_manager`
#[derive(Debug)]
pub enum MsgToWorldManager {
    /// Sent when a new call is detected from the hardware.
    AddCall(Call),

    /// Sent when an elevator finishes serving a call.
    ServedCall(Call),

    /// Sent whenever the local elevator status changes.
    NewLocalElevatorStatus(LocalElevatorStatus),

    /// Sent when a status update from a remote elevator is received.
    NewRemoteElevatorStatus(ElevatorStatus),

    /// Sent when a remote elevator is detected as disconnected.
    AddDisconnectedElevator(ElevatorId),

    /// Sent when a previously disconnected elevator reconnects.
    RemoveDisconnectedElevator(ElevatorId),
}