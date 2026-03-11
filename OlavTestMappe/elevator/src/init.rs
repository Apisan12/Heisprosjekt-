//Lage unik ID for alle calls
//Fikse sletting av ordre**

use driver_rust::elevio::elev::{self as e, DIRN_DOWN, DIRN_STOP};
use mac_address::get_mac_address;
use tokio::sync::{mpsc, watch};
use tokio::time::{sleep, Duration};

use crate::config::*;
use crate::messages::{
    CallList, ElevatorStatus, MsgToCallManager, MsgToElevatorManager, MsgToWorldManager, NodeId, MsgToFaultHandler
};

use crate::driver::input;
use crate::network::network::network_manager;
use crate::network::world_view;
use crate::network::network::recover_startup_state;
use crate::calls::call_manager;
use crate::elevator::elevator::elevator_manager;
use crate::fault_handler;

use crate::network::network::*;
use tokio::task::JoinHandle;

pub struct TaskHandles {
    pub handles: Vec<JoinHandle<()>>,
    pub shutdown_tx: watch::Sender<bool>,
}


#[derive(Debug)]
pub struct Channels {
    pub tx_call_manager: mpsc::Sender<MsgToCallManager>,
    pub rx_call_manager: mpsc::Receiver<MsgToCallManager>,
    pub tx_elevator_manager: mpsc::Sender<MsgToElevatorManager>,
    pub rx_elevator_manager: mpsc::Receiver<MsgToElevatorManager>,
    pub tx_world_manager: mpsc::Sender<MsgToWorldManager>,
    pub rx_world_manager: mpsc::Receiver<MsgToWorldManager>,
    pub tx_network: watch::Sender<ElevatorStatus>,
    pub rx_network: watch::Receiver<ElevatorStatus>,
    pub tx_fault: mpsc::Sender<MsgToFaultHandler>,
    pub rx_fault: mpsc::Receiver<MsgToFaultHandler>,
}

impl Channels {
    pub fn new(initial_status: ElevatorStatus) -> Self {
        let (tx_call_manager, rx_call_manager) = mpsc::channel::<MsgToCallManager>(32);
        let (tx_elevator_manager, rx_elevator_manager) = mpsc::channel::<MsgToElevatorManager>(32);
        let (tx_world_manager, rx_world_manager) = mpsc::channel::<MsgToWorldManager>(32);
        let (tx_network, rx_network) = watch::channel(initial_status);
        let (tx_fault, rx_fault) = mpsc::channel::<MsgToFaultHandler>(32);

        Self {
            tx_call_manager,
            rx_call_manager,
            tx_elevator_manager,
            rx_elevator_manager,
            tx_world_manager,
            rx_world_manager,
            tx_network,
            rx_network,
            tx_fault,
            rx_fault,
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
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    [0, 0, 0, 0, 0, n]
}

#[derive(Debug)]
pub struct BootContext {
    pub node_id: NodeId,
    pub elevator: e::Elevator,
    pub floor: u8,
    pub initial_status: ElevatorStatus,
    pub channels: Channels,
}

pub async fn boot() -> std::io::Result<BootContext> {
    println!("Starting boot");

    
    if !test_network_self_send().await {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotConnected,
            "network self-test failed",
        ));
    }

    // USE THIS WHEN PARSING THREE IDS ON ONE COMPUTER
    // Simulator slot (only used for port selection) when runnning several instances local
    let node_id = parse_id();

    // USE THIS WHEN USING THREE DIFFERENT COMPUTERS
    // Real elevator identity (MAC address)
    // let node_id = get_mac_node_id();

    println!("Node ID (MAC): {:?}", node_id);

    // Connect to elevator driver
    let elevator = init_elevator(node_id)?;

    // Find initial floor
    let floor = initial_floor(&elevator)
        .await
        .expect("failed to determine initial floor");

    println!("Initial floor: {}", floor);

    // Recover cab calls from network broadcasts
    let recovered_cab_calls = recover_startup_state(node_id).await;

    println!("Recovered cab calls: {}", CallList(&recovered_cab_calls));

    // Create initial elevator status
    let mut initial_status = ElevatorStatus::new(node_id, floor);

    initial_status.cab_calls = recovered_cab_calls.clone();
    initial_status.known_cab_calls = recovered_cab_calls;

    // Create communication channels
    let channels = Channels::new(initial_status.clone());

    let bootCtx = BootContext {
    node_id,
    elevator,
    floor,
    initial_status,
    channels,
    };

    // println!("BootContext: {:?}", bootCtx);

    println!("Boot finished");

    Ok(bootCtx)
}


/// Initialize elevator driver connection and return the Elevator handle.
pub fn init_elevator(slot: NodeId) -> std::io::Result<e::Elevator> {
    let port = BASE_ELEVATOR_PORT + slot[5] as u32;
    let addr = format!("localhost:{}", port);

    println!("Init_elevator port: {}", port);

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
    tx_system: mpsc::Sender<SystemCommand>,
) -> TaskHandles {
    println!("Starting tasks");

    let Channels {
        tx_call_manager,
        rx_call_manager,
        tx_elevator_manager,
        rx_elevator_manager,
        tx_world_manager,
        rx_world_manager,
        tx_network,
        rx_network,
        tx_fault,
        rx_fault,
    } = channels;

    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    let mut handles = Vec::new();

    input::spawn_input_thread(
        elev_id,
        elevator.clone(),
        tx_world_manager.clone(),
        tx_elevator_manager.clone(),
        ELEV_POLL,
        shutdown_rx.clone(),
    );

    handles.push(tokio::spawn(network_manager(
        rx_network.clone(),
        tx_world_manager.clone(),
        shutdown_rx.clone(),
    )));

    handles.push(tokio::spawn(call_manager::call_manager(
        elev_id,
        elevator.clone(),
        rx_call_manager,
        tx_elevator_manager.clone(),
        shutdown_rx.clone(),
    )));

    handles.push(tokio::spawn(world_view::world_manager(
        elev_id,
        initial_elev_status,
        rx_world_manager,
        tx_call_manager.clone(),
        tx_network.clone(),
        shutdown_rx.clone(),
    )));

    handles.push(tokio::spawn(crate::fault_handler::fault_handler(
        rx_fault,
        tx_system,
    )));

    handles.push(tokio::spawn(elevator_manager(
        elevator.clone(),
        floor,
        rx_elevator_manager,
        tx_world_manager.clone(),
        shutdown_rx.clone(),
    )));

    println!("Tasks started successfully");

    TaskHandles { handles, shutdown_tx }
}