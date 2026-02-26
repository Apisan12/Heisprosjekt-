use crate::{config::ELEV_NUM_FLOORS, messages::{Call, PeerState}, orders::order_manager};



// pub fn test_world_realistic() -> order_manager::WorldView {
//     use std::collections::{HashMap, HashSet};

//     let mut peers = HashMap::new();

//     peers.insert(1, PeerState {
//         id: 1,
//         behaviour: "moving".into(),
//         floor: 1,
//         direction: "up".into(),
//         cab_requests: vec![false, true, false, false],
//         hall_calls: vec![],
//     });

//     peers.insert(2, PeerState {
//         id: 2,
//         behaviour: "idle".into(),
//         floor: 3,
//         direction: "stop".into(),
//         cab_requests: vec![false; ELEV_NUM_FLOORS as usize],
//         hall_calls: vec![],
//     });

//     let mut hall_calls = HashSet::new();
//     hall_calls.insert(Call { id: 1, floor: 0, call_type: 0 });
//     hall_calls.insert(Call { id: 2, floor: 2, call_type: 1 });

//     order_manager::WorldView {
//         my_id: 1,
//         hall_calls,
//         my_cab_calls: HashSet::new(),
//         my_assigned: HashSet::new(),
//         peers,
//     }
// }

// pub fn test_world_stress() -> order_manager::WorldView {
//     use std::collections::{HashMap, HashSet};

//     let mut peers = HashMap::new();

//     peers.insert(1, PeerState {
//         id: 1,
//         behaviour: "moving".into(),
//         floor: 0,
//         direction: "up".into(),
//         cab_requests: vec![true, false, false, false],
//         hall_calls: vec![],
//     });

//     peers.insert(2, PeerState {
//         id: 2,
//         behaviour: "doorOpen".into(),
//         floor: 2,
//         direction: "stop".into(),
//         cab_requests: vec![false, false, true, false],
//         hall_calls: vec![],
//     });

//     peers.insert(3, PeerState {
//         id: 3,
//         behaviour: "idle".into(),
//         floor: 3,
//         direction: "stop".into(),
//         cab_requests: vec![false, false, false, false],
//         hall_calls: vec![],
//     });

//     let mut hall_calls = HashSet::new();
//     hall_calls.insert(Call { id: 30, floor: 0, call_type: 0 });
//     hall_calls.insert(Call { id: 31, floor: 1, call_type: 0 });
//     hall_calls.insert(Call { id: 32, floor: 2, call_type: 1 });
//     hall_calls.insert(Call { id: 33, floor: 3, call_type: 1 });

//     order_manager::WorldView {
//         my_id: 1,
//         hall_calls,
//         my_cab_calls: HashSet::new(),
//         my_assigned: HashSet::new(),
//         peers,
//     }
// }