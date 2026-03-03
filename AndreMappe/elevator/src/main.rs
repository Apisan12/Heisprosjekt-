mod orders;
mod network;
mod driver;
mod config;
mod messages;
mod fsm;
mod tests;
mod init;

use driver_rust::elevio::elev::{self as e, DIRN_DOWN, DIRN_STOP};
use tokio::sync::{mpsc, watch};
use messages::{ElevState, MsgToCallManager, MsgToFsm, Call};
use orders::call_manager;
use orders::assigner;
use network::network::{create_socket};
use driver::pollers::{spawn_input_pollers};
use driver::bridge::driver_bridge;
use fsm::fsm as f;
use crate::messages::NodeId;

use crate::config::*;
// fn main() {
//     let world = test_world_realistic();
//     assigner::run_assigner(&world);

//     let world2 = test_world_stress();
//     assigner::run_assigner(&world2);
// }


#[tokio::main]
async fn main() -> std::io::Result<()> {
    // ID
    // Velger id med å kjøre "cargo run --id"
    // eksempel cargo run --1
    let slot_id: u8 = init::parse_id();
    let node_id: NodeId = init::get_mac_node_id();
    // Initialisere en heis
    let elevator = init::init_elevator(slot_id)?;
    // Kobler til en heis server som bruker ID for å ha forskjellige port

    // Gir heisen en start etasje, kjører ned til nærmeste etasje hvis den står mellom etasjer
    let floor = init::initial_floor(&elevator).expect("failed to determine initial floor");

    // Lager en initial peer state som brukes som en "mal" til peerstate watch channelen
    // Brukes også til å initialisere order_manager med denne som sin peer_state.
    let initial_peer_state = ElevState::new(node_id, floor);


    // Channels
    let (tx_manager, rx_manager, tx_fsm, rx_fsm, tx_peerstate, rx_peerstate) = init::init_channels(initial_peer_state.clone());

    // UDP socket
    // Lager UDP socket og tilater broadcast
    let socket = create_socket(NETWORK_PORT);
    socket.set_broadcast(true).unwrap();
    // UDP socket

    init::spawn_tasks(
        slot_id,
        elevator.clone(),
        initial_peer_state.clone(),
        socket,
        tx_manager,
        rx_manager,
        tx_fsm,
        rx_fsm,
        tx_peerstate,
        rx_peerstate,
    );    

    // Loop for å holde main igang
    loop { tokio::time::sleep(std::time::Duration::from_secs(60)).await; }
    // Loop for å holde main igang

}