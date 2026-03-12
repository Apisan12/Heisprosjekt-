//! Entry point for the distributed elevator controller.
//!
//! This module initializes the local elevator, starts all runtime tasks,
//! and then keeps the async runtime alive for the lifetime of the program.

mod config;
mod driver;
mod elevator;
mod init;
mod messages;
mod network;
mod calls;
use calls::assigner;

#[tokio::main]
async fn main() -> std::io::Result<()> {

    // Perform startup initialization, including hardware setup,
    // network/node identification, and creation of shared channels/state.
    let boot = init::boot().await?;

    // These tasks handle the elevator logic and communication for the node.
    init::spawn_tasks(
        boot.elevator_id,
        boot.driver,
        boot.initial_status,
        boot.floor,
        boot.channels,
    );

    // Keep the Tokio runtime alive indefinitely.
    std::future::pending::<()>().await;
    Ok(())

}
