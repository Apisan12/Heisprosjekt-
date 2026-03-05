//Lage unik ID for alle calls
//Fikse sletting av ordre**

use driver_rust::elevio::elev::{self as e, DIRN_DOWN, DIRN_STOP};
use mac_address::get_mac_address;
use tokio::sync::{mpsc, watch};

use crate::config::*;
use crate::messages::{
    ElevStatus, MsgToCallManager, MsgToFsm, MsgToWorldView, NodeId,
};

use crate::driver::input;
use crate::fsm::fsm as f;
use crate::network::network::network_manager;
use crate::network::world_view;
use crate::orders::call_manager;

//Finds MAC address
pub fn get_mac_node_id() -> NodeId {
    let mac = get_mac_address()
        .expect("failed to access network interfaces")
        .expect("no MAC address found");

    mac.bytes()
}

/// Parse elevator id from CLI args.
/// Expects: cargo run -- <id>
pub fn parse_id() -> NodeId {
    let n: u8 = std::env::args()
        .nth(1)
        .expect("missing id")
        .parse()
        .expect("id must be number");

    [0, 0, 0, 0, 0, n]
}

/// Initialize elevator driver connection and return the Elevator handle.
pub fn init_elevator(id: NodeId) -> std::io::Result<e::Elevator> {
    let port = BASE_ELEVATOR_PORT + id[5] as u32;
    let addr = format!("localhost:{}", port);

    let elevator = e::Elevator::init(&addr, ELEV_NUM_FLOORS)?;
    println!("Elevator started:\n{:#?}", elevator);

    Ok(elevator)
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
    initial_elev_status: ElevStatus,
) -> (
    mpsc::Sender<MsgToCallManager>,
    mpsc::Receiver<MsgToCallManager>,
    mpsc::Sender<MsgToFsm>,
    mpsc::Receiver<MsgToFsm>,
    mpsc::Sender<MsgToWorldView>,
    mpsc::Receiver<MsgToWorldView>,
    watch::Sender<ElevStatus>,
    watch::Receiver<ElevStatus>,
) {
    let (tx_manager_msg, rx_manager_msg) = mpsc::channel::<MsgToCallManager>(32);
    let (tx_fsm_msg, rx_fsm_msg) = mpsc::channel::<MsgToFsm>(32);
    let (tx_world_view_msg, rx_world_view_msg) = mpsc::channel::<MsgToWorldView>(32);
    let (tx_network, rx_network) = watch::channel(initial_elev_status);

    (
        tx_manager_msg,
        rx_manager_msg,
        tx_fsm_msg,
        rx_fsm_msg,
        tx_world_view_msg,
        rx_world_view_msg,
        tx_network,
        rx_network,
    )
}

/// Spawns all tasks.
pub fn spawn_tasks(
    elev_id: NodeId,
    elevator: e::Elevator,
    initial_elev_status: ElevStatus,
    floor: u8,
    tx_manager_msg: mpsc::Sender<MsgToCallManager>,
    rx_manager_msg: mpsc::Receiver<MsgToCallManager>,
    tx_fsm_msg: mpsc::Sender<MsgToFsm>,
    rx_fsm_msg: mpsc::Receiver<MsgToFsm>,
    tx_world_view_msg: mpsc::Sender<MsgToWorldView>,
    rx_world_view_msg: mpsc::Receiver<MsgToWorldView>,
    tx_network: watch::Sender<ElevStatus>,
    rx_network: watch::Receiver<ElevStatus>,
) {
    // INPUT
    input::spawn_input_thread(
        elev_id,
        elevator.clone(),
        tx_manager_msg.clone(),
        tx_fsm_msg.clone(),
        ELEV_POLL,
    );

    // NETWORK (UdpSocket isn't Clone, so use try_clone for the second task)
    tokio::spawn(network_manager(
        rx_network.clone(),
        tx_world_view_msg.clone(),
    ));

    // ORDER MANAGER
    tokio::spawn(call_manager::call_manager(
        elev_id,
        elevator.clone(),
        rx_manager_msg,
        tx_world_view_msg.clone(),
        tx_fsm_msg.clone(),
    ));

    // WORLD MANAGER
    tokio::spawn(world_view::world_manager(
        elev_id,
        initial_elev_status,
        rx_world_view_msg,
        tx_manager_msg.clone(),
        tx_network.clone(),
    ));

    // FSM
    tokio::spawn(f::fsm(
        elevator.clone(),
        f::ElevatorState::Idle,
        floor,
        rx_fsm_msg,
        tx_manager_msg.clone(),
        tx_fsm_msg.clone(),
        tx_world_view_msg.clone(),
    ));
}
