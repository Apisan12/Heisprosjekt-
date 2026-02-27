use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use crate::config::ELEV_NUM_FLOORS;

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
pub struct PeerState {
    pub id: NodeId,
    pub behaviour: Behaviour,
    pub floor: u8,
    pub direction: Direction,
    pub cab_requests: Vec<bool>,
    pub hall_calls: HashSet<Call>,
    pub known_hall_calls: HashSet<Call>,
    pub finished_hall_calls: HashSet<Call>,
}

impl PeerState {
    pub fn new(id: NodeId, floor: u8) -> Self {
        Self {
            id,
            behaviour: Behaviour::Idle,
            floor,
            direction: Direction::Stop,
            cab_requests: vec![false; ELEV_NUM_FLOORS as usize],
            hall_calls: HashSet::new(),
            known_hall_calls: HashSet::new(),
            finished_hall_calls: HashSet::new(),
        }
    }
}


#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LocalState {
    pub behaviour: Behaviour,
    pub floor: u8,
    pub direction: Direction,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CallId {
    pub origin: NodeId,
    pub seq: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Call {
    pub id: CallId,
    pub floor: u8,
    pub call_type: u8,
}

#[derive(Debug)]
pub enum FsmMsg {
    AtFloor(u8),
    OrdersUpdated(Vec<[bool; 3]>),
    DoorTimeout,
}

#[derive(Debug)]
pub enum ManagerMsg {
    NewCall(Call),
    NetUpdate(PeerState),
    LocalUpdate(LocalState),
    CallFinished(Call),
}
