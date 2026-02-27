use std::collections::{HashMap, HashSet};
use crate::messages::{Call, PeerState, LocalState, NodeId};



pub struct WorldView {
    pub pending_calls: HashSet<Call>,
    pub commited_calls: HashSet<Call>,
    pub finished_calls: HashSet<Call>,
    pub peers: HashMap<NodeId, PeerState>,
}

impl WorldView {
    pub fn new() -> Self {
        Self {
            pending_calls: HashSet::new(),
            commited_calls: HashSet::new(),
            finished_calls: HashSet::new(),
            peers: HashMap::new(),
        }
    }
}