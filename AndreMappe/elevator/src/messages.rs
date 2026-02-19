use serde::{Serialize, Deserialize};
use std::collections::HashSet;


#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PeerState {
    pub id: u8,
    pub behaviour: String,
    pub floor: u8,
    pub direction: String,
    pub cab_requests: Vec<bool>,
    pub hall_calls: Vec<[bool; 2]>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LocalState {
    pub behaviour: String,
    pub floor: u8,
    pub direction: String,
}


#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Call {
    pub id: u8,
    pub floor: u8,
    pub call_type: u8,
}

#[derive(Debug)]
pub enum FsmMsg {
    AtFloor(u8),
    OrdersUpdated(Vec<[bool;3]>),
    DoorTimeout,
} 

#[derive(Debug)]
pub enum ManagerMsg {
    NewCall(Call),
    NetUpdate(PeerState),
    LocalUpdate(LocalState),
}