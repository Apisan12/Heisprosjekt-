//Lage unik ID for alle calls
//Fikse sletting av ordre**

use driver_rust::elevio::elev::{self as e, DIRN_DOWN, DIRN_STOP};
use mac_address::get_mac_address;
use tokio::sync::{mpsc, watch};
use tokio::time::{sleep, Duration};

use crate::config::*;
use crate::messages::{
    ElevatorStatus, MsgToCallManager, MsgToFsm, MsgToWorldView, NodeId,
};

use crate::driver::input;
use crate::network::network::network_manager;
use crate::network::world_view;
use crate::orders::call_manager;

pub struct Channels {
    pub tx_manager: mpsc::Sender<MsgToCallManager>,
    pub rx_manager: mpsc::Receiver<MsgToCallManager>,
    pub tx_fsm: mpsc::Sender<MsgToFsm>,
    pub rx_fsm: mpsc::Receiver<MsgToFsm>,
    pub tx_world: mpsc::Sender<MsgToWorldView>,
    pub rx_world: mpsc::Receiver<MsgToWorldView>,
    pub tx_net: watch::Sender<ElevatorStatus>,
    pub rx_net: watch::Receiver<ElevatorStatus>,
}

impl Channels {
    pub fn new(initial_status: ElevatorStatus) -> Self {
        let (tx_manager, rx_manager) = mpsc::channel::<MsgToCallManager>(32);
        let (tx_fsm, rx_fsm) = mpsc::channel::<MsgToFsm>(32);
        let (tx_world, rx_world) = mpsc::channel::<MsgToWorldView>(32);
        let (tx_net, rx_net) = watch::channel(initial_status);

        Self {
            tx_manager,
            rx_manager,
            tx_fsm,
            rx_fsm,
            tx_world,
            rx_world,
            tx_net,
            rx_net,
        }
    }
}



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
pub async fn initial_floor(elev: &e::Elevator) -> Option<u8> {
    if let Some(floor) = elev.floor_sensor() {
        return Some(floor);
    }

    elev.motor_direction(DIRN_DOWN);

    loop {
        if let Some(floor) = elev.floor_sensor() {
            elev.motor_direction(DIRN_STOP);
            return Some(floor);
        }
        sleep(Duration::from_millis(10)).await;
    }
}


pub fn spawn_tasks(
    elev_id: NodeId,
    elevator: e::Elevator,
    initial_elev_status: ElevatorStatus,
    floor: u8,
    channels: Channels,
) {
    let Channels {
        tx_manager: tx_manager_msg,
        rx_manager: rx_manager_msg,
        tx_fsm: tx_fsm_msg,
        rx_fsm: rx_fsm_msg,
        tx_world: tx_world_view_msg,
        rx_world: rx_world_view_msg,
        tx_net: tx_network,
        rx_net: rx_network,
    } = channels;

    // INPUT
    input::spawn_input_thread(
        elev_id,
        elevator.clone(),
        tx_manager_msg.clone(),
        tx_fsm_msg.clone(),
        ELEV_POLL,
    );

    // NETWORK
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


