mod config;
mod driver;
mod elevator;
mod init;
mod messages;
mod network;
mod calls;
mod tests;
mod fault_handler;

use crate::messages::NodeId;
use messages::ElevatorStatus;
use calls::assigner;


#[tokio::main]
async fn main() -> std::io::Result<()> {


    // Boot
    let boot = init::boot().await?;

    // Spawn tasks
    init::spawn_tasks(
        boot.node_id,
        boot.elevator,
        boot.initial_status,
        boot.floor,
        boot.channels,
    );

    // Keep main alive
    std::future::pending::<()>().await;
    Ok(())

}
