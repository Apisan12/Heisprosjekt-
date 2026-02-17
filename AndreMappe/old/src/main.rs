mod messages;
mod fsm;
mod orders;
mod io;

use std::time::Duration;
use driver_rust::elevio::elev as e;
use driver_rust::elevio::poll;
use crossbeam_channel::{self as cbc, select};

use crate::io::{spawn_pollers, PollReceivers};
use crate::fsm::{Fsm, ElevatorState};
use crate::orders::{OrderManager};
use crate::messages::{Command,FsmEvent};

fn main() -> std::io::Result<()> {
let elev_num_floors = 4;
    let elevator = e::Elevator::init("localhost:15657", elev_num_floors)?;
    println!("Elevator started:\n{:#?}", elevator);

    let poll_period = Duration::from_millis(25);
    let pollers = spawn_pollers(elevator.clone(), poll_period);
    let (fsm_event_tx, fsm_event_rx) = cbc::unbounded::<FsmEvent>();
    let (cmd_tx, cmd_rx) = cbc::unbounded::<Command>();

    let mut fsm = Fsm::new(elevator.clone(), fsm_event_tx.clone());
    let mut orders = OrderManager::new(elevator.clone(), cmd_tx.clone());

    loop {
        select! {
            recv(pollers.call_button) -> a => {
                let call = a.unwrap();
                println!("{:#?}", call);
                orders.new_call(call);
            }

            recv(pollers.floor_sensor) -> a => {
                let floor = a.unwrap();
                println!("Floor: {:#?}", floor);
                fsm.handle_event(FsmEvent::AtFloor(floor));
            }

            recv(fsm_event_rx) -> a => {
                let event = a.unwrap();
                match event {
                    FsmEvent::AtFloor(_) | FsmEvent::DoorTimeout => {
                        fsm.handle_event(event);
                    }

                    FsmEvent::Idle => {
                        orders.handle_fsm_event(event);
                    }

                    _ => {}
                }
                
            }

            recv(cmd_rx) -> a => {
                let command = a.unwrap();
                fsm.handle_command(command);
            }
        }
    }
}
