use tokio::sync::mpsc;
use crate::messages::{ManagerMsg,FsmMsg};
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
    mut rx: mpsc::Receiver<FsmMsg>,
    tx_manager: mpsc::Sender<ManagerMsg>,
) {
    
    // Tar imot events
    while let Some(msg) = rx.recv().await {

        match msg {
            FsmMsg::AtFloor(floor) => {
                should_stop();
            }
            FsmMsg::OrdersUpdated(orders) => {
                next_stop();
            }
            FsmMsg::DoorTimeout => {
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