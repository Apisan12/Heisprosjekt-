use tokio::sync::mpsc;

use crate::messages::{MsgToFaultHandler, SystemCommand};

pub async fn fault_handler(
    mut rx_fault: mpsc::Receiver<MsgToFaultHandler>,
    tx_system: mpsc::Sender<SystemCommand>,
) {
    let mut restarting = false;

    while let Some(msg) = rx_fault.recv().await {
        match msg {
            MsgToFaultHandler::FaultDetected => {
                if !restarting {
                    restarting = true;
                    println!("Fault detected, requesting restart");
                    let _ = tx_system.send(SystemCommand::Restart).await;
                }
            }
        }
    }
}

