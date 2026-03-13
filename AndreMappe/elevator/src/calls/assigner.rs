//! Hall calls assignment module.
//!
//! This module provides the interface between the elevator system
//! and the external `hall_request_assigner` executable. The assigner takes
//! the current world state and all active hall calls, then computes which
//! elevator should handle each hall request.
//!
//! The process works as follows:
//!
//! 1. Gather the current state of all elevators from the `WorldView`.
//! 2. Convert active hall calls into the matrix format expected by the assigner.
//! 3. Serialize the input data to JSON.
//! 4. Execute the `hall_request_assigner` program with the JSON input.
//! 5. Parse the JSON output to determine which hall calls are assigned to
//!    the current elevator.
//! 6. Convert the resulting assignment matrix back into a `HashSet<Call>`.
//!
//! Only hall calls are distributed by the assigner. Cab calls are always
//! handled by the elevator where they originate.

use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::process::Command;
use std::path::PathBuf;
use crate::messages::{Call, ElevatorStatus, ElevatorId};
use crate::network::world_view::WorldView;
use crate::config::ELEVATOR_NUM_FLOORS;
use crate::elevator::elevator::{Behaviour, Direction};

/// Input struct sent to the `hall_request_assigner` program
/// 
/// The naming of the fields must match the format expected by the
///  `hall_request_assigner`. The struct is serialized to JSON before 
/// being passed to the assigner.
#[derive(Serialize)]
struct AssignerInput {
    /// Matrix representing the active hall calls.
    #[serde(rename = "hallRequests")]
    hall_requests: Vec<[bool; 2]>,

    /// Current states of all the elevators in the system
    /// 
    /// THe key is a string formatted as `"id_<ElevatorId>"`
    states: HashMap<String, AssignerState>,
}

impl AssignerInput {

    /// Builds the input structure sent to the hall request assigner.
    pub fn new(world: &WorldView, active_calls: &HashSet<Call>) -> Self {
        Self {
            hall_requests: calls_to_assigner_matrix(active_calls),
            states: world.assigner_states(),
        }
    }
}

/// Output structure returned by the assigner.
/// 
/// Each elevator ID maps to a hall call matrix indicating which calls
/// that elevator should serve.
type AssignerOutput = HashMap<String, Vec<[bool; 3]>>;

/// Elevator state representation used by the assigner.
/// 
/// The naming of the fields must match the format expected by the
/// `hall_request_assigner`.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignerState {
    pub behaviour: Behaviour,
    pub floor: u8,
    pub direction: Direction,

    /// Active cab calls for the elevator.
    /// 
    /// Each index correspons to a floor.
    pub cab_requests: Vec<bool>,
}

impl AssignerState {

    /// Constructs an `AssignerState` from the status of a specific elevator.
    /// 
    /// Also uses the `WorldView` to check for active cab calls since they
    /// need to be known by other elevators to be active. Then converts
    /// the cab calls to the assigner format.
    pub fn from_elevator(world: &WorldView, elevator: &ElevatorStatus) -> Self {
        let mut cab_requests = vec![false; ELEVATOR_NUM_FLOORS as usize];

        for call in world.active_cab_calls(&elevator.elevator_id) {
            if call.floor < ELEVATOR_NUM_FLOORS {
                cab_requests[call.floor as usize] = true;
            }
        }
        
        Self {
            behaviour: elevator.behaviour,
            floor: elevator.floor,
            direction: elevator.direction,
            cab_requests,
        }
    }

}

/// Runs the hall request assigner and returns the hall calls assigned
/// to the current elevator.
pub fn run_assigner(
    mut world: WorldView,
    active_calls: &HashSet<Call>,
    elevator_id: ElevatorId,
) -> HashSet<Call> {

    // Removes faulty elevators, so they dont get any assignments.
    world.remove_faulty_elevators();

    let input = AssignerInput::new(&world, &active_calls);
    let json_input = serde_json::to_string(&input).unwrap();

    let assigner_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("hall_request_assigner");

    let output = Command::new(assigner_path)
        .arg("--input")
        .arg(&json_input)
        .arg("--includeCab")
        .output()
        .expect("failed to run hall_request_assigner");


    if !output.status.success() {
        return HashSet::new();
    }

    let stdout = String::from_utf8(output.stdout).unwrap();

    let assigned_matrix: AssignerOutput = 
        serde_json::from_str(&stdout).expect("invalid assigner output");

    let my_key = format!("id_{elevator_id:?}");

    let my_assigned_matrix = match assigned_matrix
        .get(&my_key){
            Some(matrix) => matrix,
            None => return HashSet::new(),
        };

    assigned_matrix_to_calls(&active_calls, my_assigned_matrix)

}

/// Converts hall calls into the matrix format expected by the assigner.
fn calls_to_assigner_matrix(active_calls: &HashSet<Call>) -> Vec<[bool; 2]> {
    let mut matrix = vec![[false; 2]; ELEVATOR_NUM_FLOORS as usize];

    for call in active_calls {
        if call.call_type < 2 {
            matrix[call.floor as usize][call.call_type as usize] = true;
        }
    }
    matrix
}

/// Converts an assigner matrix result back into a `HashSet<Call>`.
fn assigned_matrix_to_calls(
    active_calls: &HashSet<Call>,
    matrix: &Vec<[bool; 3]>,
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