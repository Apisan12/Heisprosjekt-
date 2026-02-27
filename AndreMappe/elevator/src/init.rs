//Lage unik ID for alle calls
//Fikse sletting av ordre*

use driver_rust::elevio::elev::{self as e, DIRN_DOWN, DIRN_STOP};
use tokio::sync::{mpsc, watch};

use crate::messages::{PeerState, ManagerMsg, FsmMsg};
use crate::config::*;

use crate::driver::pollers::spawn_input_pollers;
use crate::driver::bridge::driver_bridge;
use crate::network::network::{peer_state_receiver, peer_state_sender};
use crate::orders::order_manager;
use crate::fsm::fsm as f;

/// Parse elevator id from CLI args.
/// Expects: cargo run -- <id>
pub fn parse_id() -> u8 {
    std::env::args()
        .nth(1)
        .expect("missing id")
        .parse()
        .expect("id must be number")
}

/// Initialize elevator driver connection and return the Elevator handle.
pub fn init_elevator(id: u8) -> std::io::Result<e::Elevator> {
    let port = BASE_ELEVATOR_PORT + id as u32;
    let addr = format!("localhost:{}", port);

    let elevator = e::Elevator::init(&addr, ELEV_NUM_FLOORS)?;
    println!("Elevator started:\n{:#?}", elevator);

    Ok(elevator)
}

/// Create the initial PeerState watch channel.
/// This also makes sure we have a valid starting floor.
pub fn init_peerstate_channel(
    id: u8,
    elevator: &e::Elevator,
) -> (watch::Sender<PeerState>, watch::Receiver<PeerState>) {
    let floor = initial_floor(elevator).expect("failed to determine initial floor");

    watch::channel(PeerState {
        id,
        behaviour: String::from("idle"),
        floor,
        direction: String::from("stop"),
        cab_requests: vec![false; ELEV_NUM_FLOORS as usize],
        hall_calls: vec![[false, false]; ELEV_NUM_FLOORS as usize],
    })
}

/// Return current floor; if between floors, drive down until a floor is detected.
pub fn initial_floor(elev: &e::Elevator) -> Option<u8> {
    if let Some(floor) = elev.floor_sensor() {
        return Some(floor);
    }

    elev.motor_direction(DIRN_DOWN);

    loop {
        if let Some(floor) = elev.floor_sensor() {
            elev.motor_direction(DIRN_STOP);
            return Some(floor);
        }
    }
}

/// Creates all channels and returns them as a tuple.
pub fn init_channels(
    id: u8,
    elevator: &e::Elevator,
) -> (
    mpsc::Sender<ManagerMsg>,
    mpsc::Receiver<ManagerMsg>,
    mpsc::Sender<FsmMsg>,
    mpsc::Receiver<FsmMsg>,
    watch::Sender<PeerState>,
    watch::Receiver<PeerState>,
) {
    let (tx_manager, rx_manager) = mpsc::channel::<ManagerMsg>(32);
    let (tx_fsm, rx_fsm) = mpsc::channel::<FsmMsg>(32);
    let (tx_peerstate, rx_peerstate) = init_peerstate_channel(id, elevator);

    (tx_manager, rx_manager, tx_fsm, rx_fsm, tx_peerstate, rx_peerstate)
}

/// Spawns all tasks.
pub fn spawn_tasks(
    id: u8,
    elevator: e::Elevator,
    socket: std::net::UdpSocket,
    tx_manager: mpsc::Sender<ManagerMsg>,
    rx_manager: mpsc::Receiver<ManagerMsg>,
    tx_fsm: mpsc::Sender<FsmMsg>,
    rx_fsm: mpsc::Receiver<FsmMsg>,
    tx_peerstate: watch::Sender<PeerState>,
    rx_peerstate: watch::Receiver<PeerState>,
) {
    // INPUT
    let pollers = spawn_input_pollers(elevator.clone(), ELEV_POLL);
    tokio::spawn(driver_bridge(
        id,
        pollers,
        tx_manager.clone(),
        tx_fsm.clone(),
    ));

    // NETWORK (UdpSocket isn't Clone, so use try_clone for the second task)
    let socket_rx = socket.try_clone().expect("socket try_clone failed");
    tokio::spawn(peer_state_receiver(socket_rx, tx_manager.clone()));
    tokio::spawn(peer_state_sender(socket, rx_peerstate));

    // ORDER MANAGER
    tokio::spawn(order_manager::order_manager(
        id,
        rx_manager,
        tx_peerstate,
        elevator.clone(),
    ));

    // FSM
    tokio::spawn(f::fsm(
        elevator,
        f::ElevatorState::Idle,
        rx_fsm,
        tx_manager,
    ));
}
