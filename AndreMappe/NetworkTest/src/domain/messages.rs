use serde::{Serialize, Deserialize};
use std::collections::HashSet;


#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NetState {
    pub id: u8,
    pub calls: HashSet<Call>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Call {
    pub floor: u8,
    pub call: u8,
}
