use tokio::sync::mpsc;
use crate::messages::{MsgToCallManager,MsgToFsm};
use driver_rust::elevio::elev as e;

pub enum ElevatorState {
    Idle,
    Moving,
    DoorOpen,
    Stopped,
}


pub async fn fsm(
    elevator: e::Elevator,
    state: ElevatorState,
    mut rx: mpsc::Receiver<MsgToFsm>,
    tx_manager: mpsc::Sender<MsgToCallManager>,
) {
    
    // Tar imot events
    while let Some(msg) = rx.recv().await {

        match msg {
            MsgToFsm::AtFloor(floor) => {
                should_stop();
            }
            MsgToFsm::AddCall(orders) => {
                next_stop();
            }
            MsgToFsm::DoorTimeout => {
                close_door();
            }
        }
    }
}

fn should_stop() {
    todo!();
}

fn next_stop() {
    todo!();
}

fn close_door() {
    todo!();
}