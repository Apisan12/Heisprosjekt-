use std::time::Duration;
use std::thread;
use crate::messages::{Command, FsmEvent};
use driver_rust::elevio::elev as e;
use super::state::ElevatorState;
use crossbeam_channel as cbc;

pub struct Fsm {
    state: ElevatorState,
    elevator: e::Elevator,
    event_tx: cbc::Sender<FsmEvent>,
}

impl Fsm {
    pub fn new(elevator: e::Elevator, event_tx: cbc::Sender<FsmEvent>) -> Self {
        Self {
            state: ElevatorState::Idle,
            elevator,
            event_tx,
        }
    }

    pub fn handle_command(&mut self, cmd: Command) {
        match (&self.state, cmd) {
            (ElevatorState::Idle, Command::GoToFloor(target)) => {
                let current_floor = match self.elevator.floor_sensor() {
                    Some(f) => f,
                    None => return,
                };

                if target > current_floor {
                    self.elevator.motor_direction(e::DIRN_UP);
                    self.state = ElevatorState::Moving { target };
                }
                else if target < current_floor {
                    self.elevator.motor_direction(e::DIRN_DOWN);
                    self.state = ElevatorState::Moving { target };
                    
                }
                else {
                    self.open_door();
                }
            }

            (ElevatorState::Idle, Command::OpenDoor) => {
                self.open_door();
            }

            (ElevatorState::Moving { .. }, Command::Stop) => {
                self.elevator.motor_direction(e::DIRN_STOP);
                self.state = ElevatorState::Idle;
            }

            (ElevatorState::DoorOpen, Command::OpenDoor) => {
                self.restart_door_tmer();
            }

            (_, _) => {

            }
        }
    }

    pub fn handle_event(&mut self, event: FsmEvent) {
        match (&mut self.state, event) {

            (ElevatorState::Moving { target }, FsmEvent::AtFloor(floor)) => {
                self.elevator.floor_indicator(floor);

                if floor == *target {
                    self.elevator.motor_direction(e::DIRN_STOP);
                    self.open_door();

                    self.event_tx.send(FsmEvent::AtFloor(floor)).ok();
                }
            }

            (ElevatorState::DoorOpen, FsmEvent::DoorTimeout) => {
                if self.elevator.obstruction() {
                    self.restart_door_tmer();
                } else {
                    self.elevator.door_light(false);
                    self.state = ElevatorState::Idle;

                    self.event_tx.send(FsmEvent::Idle).ok();
                }
            }

            _ => {}
        }
    }

    pub fn is_idle(&self) -> bool {
        matches!(self.state, ElevatorState::Idle)
    }

    fn open_door(&mut self) {
        self.elevator.motor_direction(e::DIRN_STOP);
        self.elevator.door_light(true);
        self.elevator.call_button_light(self.elevator.floor_sensor().unwrap(), 0 , false);
        self.elevator.call_button_light(self.elevator.floor_sensor().unwrap(),1, false);
        self.elevator.call_button_light(self.elevator.floor_sensor().unwrap(),2, false);
        self.start_door_timer();
        self.state = ElevatorState::DoorOpen;
    }

    fn start_door_timer(&self) {
        let tx = self.event_tx.clone();
        println!("Starter dør timer");
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(3));
            tx.send(FsmEvent::DoorTimeout).ok();
        });
    }

    fn restart_door_tmer(&self) {
        self.start_door_timer();
    }

}