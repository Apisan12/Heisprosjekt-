mod config;
mod driver;
mod elevator;
mod init;
mod messages;
mod network;
mod orders;
mod tests;

use crate::messages::NodeId;
use messages::ElevatorStatus;
use orders::assigner;


#[tokio::main]
async fn main() -> std::io::Result<()> {
    // ID
    // Velger id med å kjøre "cargo run --id"
    // eksempel cargo run -- 1
    let slot_id: NodeId = init::parse_id();
    let _node_id: NodeId = init::get_mac_node_id();
    // Initialisere en heis
    let elevator = init::init_elevator(slot_id)?;
    // Kobler til en heis server som bruker ID for å ha forskjellige port

    // Gir heisen en start etasje, kjører ned til nærmeste etasje hvis den står mellom etasjer
    let floor = init::initial_floor(&elevator)
        .await
        .expect("failed to determine initial floor");
    // Lager en initial elev status som brukes som en "mal" til network watch channelen
    // Brukes også til å initialisere worldview med denne som sin peer_state.
    let initial_elev_status = ElevatorStatus::new(slot_id, floor, cab_calls);

    // Channels 
    let channels = init::Channels::new(initial_elev_status.clone());

    // Spawn tasks
    init::spawn_tasks(
        slot_id,
        elevator.clone(),
        initial_elev_status.clone(),
        floor,
        channels,
    );

    // Keep main alive
    std::future::pending::<()>().await;
    Ok(())

}
