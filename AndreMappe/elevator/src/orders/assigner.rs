use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::process::Command;
use crate::messages::{Behaviour, Call, Direction, ElevatorStatus, NodeId};
use crate::network::world_view::WorldView;
use crate::config::ELEV_NUM_FLOORS;


// The naming in AssignerInput corresponds to the naming that is expected
// by the hall_request_assigner.exe script.
#[derive(Serialize)]
struct AssignerInput {
    #[serde(rename = "hallRequests")]
    hall_requests: Vec<[bool; 2]>,

    states: HashMap<String, AssignerState>,
}

type AssignerOutput = HashMap<String, Vec<[bool; 2]>>;

// The naming in AssignerState corresponds to the naming that is expected
// for the states in the hall_request_assigner.exe script.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignerState {
    pub behaviour: Behaviour,
    pub floor: u8,
    pub direction: Direction,
    pub cab_requests: Vec<bool>,
}

impl AssignerState {

    pub fn from_elev(elev: &ElevatorStatus) -> Self {
        let mut cab_requests = vec![false; ELEV_NUM_FLOORS as usize];
        for call in &elev.cab_calls {
            if call.floor < ELEV_NUM_FLOORS {
                cab_requests[call.floor as usize] = true;
            }
        }
        
        Self {
            behaviour: elev.behaviour,
            floor: elev.floor,
            direction: elev.direction,
            cab_requests,
        }
    }

}

pub fn run_assigner(
    world: &WorldView,
    active_calls: &HashSet<Call>,
    elev_id: NodeId,
) -> HashSet<Call> {
    let input = build_assigner_input(world, &active_calls);

    let json_input = serde_json::to_string(&input).unwrap();

    let output = Command::new("./hall_request_assigner")
        .arg("--input")
        .arg(&json_input)
        .output()
        .expect("failed to run hall_request_assigner");

    if !output.status.success() {
        panic!("assigner failed");
    }

    let stdout = String::from_utf8(output.stdout).unwrap();

    let assigned_matrix: AssignerOutput = 
        serde_json::from_str(&stdout).expect("invalid assigner output");

    let my_key = format!("id_{elev_id:?}");
    let my_assigned_matrix = assigned_matrix
        .get(&my_key)
        .expect("assigner missing my id");

    assigned_matrix_to_calls(&active_calls, my_assigned_matrix)

}

// Lager input-JSON filen som brukes i assigner scriptet.
fn build_assigner_input(world: &WorldView, active_calls: &HashSet<Call>) -> AssignerInput {
    AssignerInput { 
        hall_requests: calls_to_assigner_matrix(active_calls),
        states: world.assigner_states(),
    }
}

// Lager Hall Call matrisen som sendes til assigner skriptet
fn calls_to_assigner_matrix(active_calls: &HashSet<Call>) -> Vec<[bool; 2]> {
    let mut matrix = vec![[false; 2]; ELEV_NUM_FLOORS as usize];

    for call in active_calls {
        if call.call_type < 2 {
            matrix[call.floor as usize][call.call_type as usize] = true;
        }
    }
    matrix
}

fn assigned_matrix_to_calls(
    active_calls: &HashSet<Call>,
    matrix: &Vec<[bool; 2]>,
) -> HashSet<Call> {

    let mut calls = HashSet::new();

    for call in active_calls {
        let floor = call.floor as usize;
        let call_type = call.call_type as usize;

        if floor < matrix.len() && call_type < 2 {
            if matrix[floor][call_type] {
                calls.insert(*call);
            }
        }
    }

    calls
}