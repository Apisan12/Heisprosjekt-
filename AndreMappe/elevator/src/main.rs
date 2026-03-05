mod config;
mod driver;
mod fsm;
mod init;
mod messages;
mod network;
mod orders;
mod tests;

use crate::messages::NodeId;
use messages::ElevStatus;
use orders::assigner;


#[tokio::main]
async fn main() -> std::io::Result<()> {
    // ID
    // Velger id med å kjøre "cargo run --id"
    // eksempel cargo run --1
    let slot_id: NodeId = init::parse_id();
    let _node_id: NodeId = init::get_mac_node_id();
    // Initialisere en heis
    let elevator = init::init_elevator(slot_id)?;
    // Kobler til en heis server som bruker ID for å ha forskjellige port

    // Gir heisen en start etasje, kjører ned til nærmeste etasje hvis den står mellom etasjer
    let floor = init::initial_floor(&elevator).expect("failed to determine initial floor");

    // Lager en initial elev status som brukes som en "mal" til network watch channelen
    // Brukes også til å initialisere worldview med denne som sin peer_state.
    let initial_elev_status = ElevStatus::new(slot_id, floor);

    // Channels
    let (
        tx_manager_msg,
        rx_manager_msg,
        tx_fsm_msg,
        rx_fsm_msg,
        tx_world_view_msg,
        rx_world_view_msg,
        tx_network,
        rx_network,
    ) = init::init_channels(initial_elev_status.clone());


    init::spawn_tasks(
        slot_id,
        elevator.clone(),
        initial_elev_status.clone(),
        floor,
        tx_manager_msg,
        rx_manager_msg,
        tx_fsm_msg,
        rx_fsm_msg,
        tx_world_view_msg,
        rx_world_view_msg,
        tx_network,
        rx_network,
    );

    // Loop for å holde main igang
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    }
    // Loop for å holde main igang
}
