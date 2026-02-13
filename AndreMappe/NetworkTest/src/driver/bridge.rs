use tokio::sync::mpsc;
use crossbeam_channel as cbc;
use crate::logic::logic_loop::LogicMsg;
use crate::domain::messages::Call;
use super::pollers::PollReceivers;
use driver_rust::elevio::poll;

pub async fn driver_bridge(
    poll_rx: PollReceivers,
    tx_logic: mpsc::Sender<LogicMsg>,
) {
    tokio::task::spawn_blocking(move || {
        loop {
            crossbeam_channel::select! {

                recv(poll_rx.call_button) -> msg => {
                    if let Ok(btn) = msg {
                        let call = Call {
                            floor: btn.floor,
                            call: btn.call as u8,
                        };
                        let _ = tx_logic.blocking_send(LogicMsg::LocalButton(call));
                    }
                }

                recv(poll_rx.floor_sensor) -> msg => {
                    if let Ok(floor) = msg {
                        // send egen LogicMsg hvis du har en
                        // let _ = tx_logic.blocking_send(LogicMsg::FloorSensor(floor));
                        println!("Floor sensor: {}", floor);
                    }
                }

                recv(poll_rx.stop_button) -> _ => {}

                recv(poll_rx.obstruction) -> _ => {}
            }
        }
    }).await.ok();
}
