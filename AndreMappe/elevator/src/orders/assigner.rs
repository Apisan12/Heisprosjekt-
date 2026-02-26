use serde::{Serialize,Deserialize};
use std::collections::HashMap;
use std::process::Command;
use crate::orders::order_manager;
use crate::config::ELEV_NUM_FLOORS;

#[derive(Serialize)]
struct AssignerInput {
    #[serde(rename = "hallRequests")]
    hall_requests: Vec<[bool; 2]>,

    states: HashMap<String, AssignerState>,
}

#[derive(Serialize)]
struct AssignerState {
    behaviour: String,
    floor: u8,
    direction: String,

    #[serde(rename = "cabRequests")]
    cab_requests: Vec<bool>,
}

pub fn run_assigner(world: &order_manager::WorldView) -> Vec<[bool; 2]> {
    let input = build_assigner_input(world);

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

    println!("Assigner output: {}", stdout);

    let assignments: HashMap<String, Vec<[bool; 2]>> =
        serde_json::from_str(&stdout).expect("failed to parse assigner output");

    let my_key = format!("id_{}", world.my_id);

    assignments
        .get(&my_key)
        .cloned()
        .unwrap_or_else(|| vec![[false; 2]; ELEV_NUM_FLOORS as usize])


}

// Lager input-JSON filen som brukes i assigner scriptet.
fn build_assigner_input(world: &order_manager::WorldView) -> AssignerInput {
    let hall_requests = hall_calls_to_matrix(world);

    let mut states = HashMap::new();

    for(id, peer) in &world.peers {
        states.insert(
            format!("id_{}", id),
            AssignerState {
                behaviour: peer.behaviour.clone(),
                floor: peer.floor,
                direction: peer.direction.clone(),
                cab_requests: peer.cab_requests.clone(),
            },
        );
    }

    AssignerInput { hall_requests, states }
}

// Lager Hall Call matrisen som sendes til assigner skriptet
fn hall_calls_to_matrix(world: &order_manager::WorldView) -> Vec<[bool; 2]> {
    let mut matrix = vec![[false; 2]; ELEV_NUM_FLOORS as usize];

    for call in &world.hall_calls {
        if call.call_type < 2 {
            matrix[call.floor as usize][call.call_type as usize] = true;
        }
    }
    matrix
}