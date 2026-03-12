//! Startup and runtime initialization for a single elevator node.
//!
//! This module is responsible for:
//! - identifying the local node
//! - connecting to the elevator driver
//! - determining the initial floor
//! - recovering persisted/distributed state at startup
//! - creating inter-task communication channels
//! - spawning the long-running async tasks that make up the system

use driver_rust::elevio::elev::{self as e, DIRN_DOWN, DIRN_STOP};
//use mac_address::get_mac_address;
use tokio::sync::{mpsc, watch};
use tokio::time::{sleep, Duration};
use crate::config::*;
use crate::messages::{CallList, ElevatorStatus, MsgToCallManager, MsgToElevatorManager, MsgToWorldManager, ElevatorId};
use crate::driver::input;
use crate::network::world_view;
use crate::calls::call_manager;
use crate::elevator::elevator::elevator_manager;

use crate::network::network::{network_manager, recover_startup_state, test_network_self_send,};

/// Collection of channels used for communication between the system's long-running tasks.
/// Each manager/task receives messages through its dedicated receiver, while shared senders are cloned and distributed where needed.
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
}


/// Create a new set of channels for inter-task communication.
/// The `initial_status` value is used to initialize the watch channel so that the networking layer immediately has a valid local state.
impl Channels {
    pub fn new(initial_status: ElevatorStatus) -> Self {
        let (tx_call_manager, rx_call_manager) = mpsc::channel::<MsgToCallManager>(32);
        let (tx_elevator_manager, rx_elevator_manager) = mpsc::channel::<MsgToElevatorManager>(32);
        let (tx_world_manager, rx_world_manager) = mpsc::channel::<MsgToWorldManager>(32);
        let (tx_network, rx_network) = watch::channel(initial_status);

        Self {
            tx_call_manager,
            rx_call_manager,
            tx_elevator_manager,
            rx_elevator_manager,
            tx_world_manager,
            rx_world_manager,
            tx_network,
            rx_network,
        }
    }
}


/// Return a node identifier derived from the machine's MAC address.
// pub fn get_mac_node_id() -> NodeId {
//     let mac = get_mac_address()
//         .expect("failed to access network interfaces")
//         .expect("no MAC address found");

//     mac.bytes()
// }

/// Parse elevator id from CLI args. Only used for simulating several elevators at the same machine.
/// Expects: cargo run -- <id> 
pub fn parse_id() -> ElevatorId {
    let n: u8 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    [0, 0, 0, 0, 0, n]
}


/// Data produced during startup and needed to launch the runtime.
#[derive(Debug)]
pub struct BootContext {
    pub node_id: ElevatorId,
    pub elevator: e::Elevator,
    pub floor: u8,
    pub initial_status: ElevatorStatus,
    pub channels: Channels,
}


/// Perform startup initialization for the local elevator node.
///
/// Startup includes:
/// - verifying that the network layer can send to itself
/// - determining id
/// - connecting to the elevator driver
/// - finding the initial floor position
/// - recovering cab-call state from the network
/// - constructing initial runtime state and channels
pub async fn boot() -> std::io::Result<BootContext> {
    println!("Starting boot");

    
    if !test_network_self_send().await {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotConnected,
            "network self-test failed",
        ));
    }

    // Use CLI-derived id when running multiple simulated elevators locally.
    let node_id = parse_id();

    // Use MAC-derived id when running on separate physical machines.
    // let node_id = get_mac_node_id();

    println!("Node ID: {:?}", node_id);

    // Connect to elevator driver
    let elevator = init_elevator(node_id)?;

    // Determine a valid initial floor reference.
    let floor = initial_floor(&elevator)
        .await
        .expect("failed to determine initial floor");

    println!("Initial floor: {}", floor);

    // Recover previously known cab calls from the distributed system.
    let recovered_cab_calls = recover_startup_state(node_id).await;

    println!("Recovered cab calls: {}", CallList(&recovered_cab_calls));

    // Build initial local elevator state.
    let mut initial_status = ElevatorStatus::new(node_id, floor);
    initial_status.cab_calls = recovered_cab_calls.clone();
    initial_status.known_cab_calls = recovered_cab_calls;

    // Create communication channels for all runtime tasks.
    let channels = Channels::new(initial_status.clone());

    let boot_ctx = BootContext {
    node_id,
    elevator,
    floor,
    initial_status,
    channels,
    };

    println!("Boot finished");

    Ok(boot_ctx)
}

/// Initialize the elevator driver connection.
/// The simulator/driver port is derived from the final byte of `slot`,
/// allowing multiple local instances to run on different ports.
/// Initialize elevator driver connection and return the Elevator handle.
pub fn init_elevator(slot: ElevatorId) -> std::io::Result<e::Elevator> {
    let port = BASE_ELEVATOR_PORT + slot[5] as u32;
    let addr = format!("localhost:{}", port);

    println!("Init_elevator port: {}", port);

    let elevator = e::Elevator::init(&addr, ELEV_NUM_FLOORS)?;
    println!("Elevator started:\n{:#?}", elevator);

    Ok(elevator)
}


/// Determine the elevator's initial floor at startup.
/// If the elevator is already aligned with a floor sensor, that floor is returned immediately. Otherwise, the elevator is driven downward until a floor sensor is reached, at which point the motor is stopped.
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

/// Spawn all long-running tasks required by the elevator node.
///
/// The runtime is split into several concurrent subsystems:
/// - input polling from the driver
/// - network communication
/// - call/order management
/// - world-state management
/// - elevator state machine / motion control
pub fn spawn_tasks(
    elev_id: ElevatorId,
    elevator: e::Elevator,
    initial_elev_status: ElevatorStatus,
    floor: u8,
    channels: Channels,
) {
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
    } = channels;

    // Poll button presses and floor events from the elevator hardware.
    input::spawn_input_thread(
        elev_id,
        elevator.clone(),
        tx_world_manager.clone(),
        tx_elevator_manager.clone(),
        ELEV_POLL,
    );

    // Broadcast local state and receive network updates.
    tokio::spawn(network_manager(
        rx_network.clone(),
        tx_world_manager.clone(),
    ));

    // Manage hall/cab calls and assign work to the elevator controller.
    tokio::spawn(call_manager::call_manager(
        elev_id,
        elevator.clone(),
        rx_call_manager,
        tx_elevator_manager.clone(),
    ));

    // Maintain the node's view of the distributed elevator world state.
    tokio::spawn(world_view::world_manager(
        elev_id,
        initial_elev_status,
        rx_world_manager,
        tx_call_manager.clone(),
        tx_network.clone(),
    ));

    // Run the local elevator finite-state machine.
    tokio::spawn(elevator_manager(
        elevator.clone(),
        floor,
        rx_elevator_manager,
        tx_world_manager.clone(),
    ));

    println!("Tasks started successfully");

}


