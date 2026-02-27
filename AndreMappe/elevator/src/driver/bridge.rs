use tokio::sync::mpsc;
use crossbeam_channel as cbc;
use crate::messages::{FsmMsg, ManagerMsg, Call, NodeId};
use super::pollers::PollReceivers;

// Tar input fra pollers og sender beskjed på Manager kanal og FSM kanal
pub async fn driver_bridge(
    id: NodeId,
    poll_rx: PollReceivers,
    tx_manager: mpsc::Sender<ManagerMsg>,
    tx_fsm: mpsc::Sender<FsmMsg>,
) {
    tokio::task::spawn_blocking(move || {
        let mut version = 1;
        loop {
            cbc::select! {

                recv(poll_rx.call_button) -> msg => {
                    if let Ok(btn) = msg {
                        
                        let call = Call {
                            id: id,
                            version: version,
                            floor: btn.floor,
                            call_type: btn.call as u8,
                        };
                        let _ = tx_manager.blocking_send(ManagerMsg::NewCall(call));
                        println!("New call: {:#?}", call);
                        version += 1;
                    }
                }

                recv(poll_rx.floor_sensor) -> msg => {
                    if let Ok(floor) = msg {
                        println!("Floor sensor: {}", floor);
                    }
                }

                recv(poll_rx.stop_button) -> _ => {}

                recv(poll_rx.obstruction) -> _ => {}
            }
        }
    }).await.ok();
}
