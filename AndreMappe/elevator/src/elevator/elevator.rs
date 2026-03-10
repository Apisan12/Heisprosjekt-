use std::collections::HashSet;

use crate::messages::{
    Behaviour, Call, Direction, MsgToCallManager, MsgToElevatorManager, MsgToWorldView,
};
use driver_rust::elevio::elev::{self, CAB, HALL_DOWN, HALL_UP};
use tokio::sync::mpsc;
use tokio::time::{Duration, sleep};

#[derive(PartialEq, Eq)]
pub enum ElevatorState {
    Idle,
    Moving,
    DoorOpen,
    Stopped,
}

impl ElevatorState {
    pub fn behaviour(&self) -> Behaviour {
        match self {
            ElevatorState::Idle => Behaviour::Idle,
            ElevatorState::Moving => Behaviour::Moving,
            ElevatorState::DoorOpen => Behaviour::DoorOpen,
            ElevatorState::Stopped => Behaviour::Idle,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LocalElevatorStatus {
    pub floor: u8,
    pub direction: Direction,
    pub behaviour: Behaviour,
    pub is_obstructed: bool,
}

impl LocalElevatorStatus {
    pub fn new(floor: u8, direction: Direction, behaviour: Behaviour, is_obstructed: bool) -> Self {
        Self {
            floor,
            direction,
            behaviour,
            is_obstructed,
        }
    }
}

struct Elevator {
    driver: elev::Elevator,
    state: ElevatorState,
    current_floor: u8,
    direction: Direction,
    calls: HashSet<Call>,
    is_obstructed: bool,
}

impl Elevator {
    fn new(driver: elev::Elevator, initial_state: ElevatorState, initial_floor: u8) -> Self {
        Self {
            driver: driver.clone(),
            state: initial_state,
            current_floor: initial_floor,
            direction: Direction::Stop,
            calls: HashSet::new(),
            is_obstructed: driver.obstruction(),
        }
    }

    fn has_calls_above(&self) -> bool {
        self.calls
            .iter()
            .any(|call| call.floor > self.current_floor)
    }

    fn has_calls_below(&self) -> bool {
        self.calls
            .iter()
            .any(|call| call.floor < self.current_floor)
    }

    fn next_direction(&self) -> Direction {
        if self.direction != Direction::Down && self.has_calls_above() {
            Direction::Up
        } else if self.direction != Direction::Up && self.has_calls_below() {
            Direction::Down
        } else {
            Direction::Stop
        }
    }

    fn service_direction(&self) -> Direction {
        if self.direction == Direction::Up && self.has_calls_above() {
            return Direction::Up;
        }

        if self.direction == Direction::Down && self.has_calls_below() {
            return Direction::Down;
        }

        // No calls ahead → serve calls at this floor
        let has_up = self
            .calls
            .iter()
            .any(|c| c.floor == self.current_floor && c.call_type == HALL_UP);

        let has_down = self
            .calls
            .iter()
            .any(|c| c.floor == self.current_floor && c.call_type == HALL_DOWN);

        if has_up {
            Direction::Up
        } else if has_down {
            Direction::Down
        } else {
            Direction::Stop
        }
    }

    fn should_serve_here(&self) -> bool {
        let dir = self.service_direction();

        self.calls.iter().any(|call| {
            call.floor == self.current_floor
                && match call.call_type {
                    CAB => true,

                    HALL_UP => dir == Direction::Up,

                    HALL_DOWN => dir == Direction::Down,

                    _ => false,
                }
        })
    }

    fn served_calls(&self) -> HashSet<Call> {
        let dir = self.service_direction();

        self.calls
            .iter()
            .filter(|call| {
                call.floor == self.current_floor
                    && match call.call_type {
                        CAB => true,

                        HALL_UP => dir == Direction::Up,

                        HALL_DOWN => dir == Direction::Down,

                        _ => false,
                    }
            })
            .cloned()
            .collect()
    }

    async fn serve_current_floor(&mut self, tx_call_manager: &mpsc::Sender<MsgToCallManager>) {
        let served = self.served_calls();

        for call in served {
            self.calls.remove(&call);
            let _ = tx_call_manager
                .send(MsgToCallManager::ServedCall(call))
                .await;
        }
    }

    fn open_door(&mut self, tx: mpsc::Sender<MsgToElevatorManager>) {
        self.driver.door_light(true);
        self.state = ElevatorState::DoorOpen;
        self.direction = Direction::Stop;

        tokio::spawn(async move {
            sleep(Duration::from_secs(3)).await;
            let _ = tx.send(MsgToElevatorManager::DoorClosed).await;
        });
    }

    fn serve_next_action(&mut self) {
        self.direction = self.next_direction();

        match self.direction {
            Direction::Up => {
                self.driver.motor_direction(elev::DIRN_UP);
                self.state = ElevatorState::Moving;
            }
            Direction::Down => {
                self.driver.motor_direction(elev::DIRN_DOWN);
                self.state = ElevatorState::Moving;
            }
            Direction::Stop => {
                self.state = ElevatorState::Idle;
            }
        }
    }

    async fn send_status_update(&self, tx_world_manager: &mpsc::Sender<MsgToWorldView>) {
        let status =
            LocalElevatorStatus::new(self.current_floor, self.direction, self.state.behaviour(), self.is_obstructed);

        let _ = tx_world_manager
            .send(MsgToWorldView::UpdateLocalElevStatus(status))
            .await;
    }
}

pub async fn elevator_manager(
    driver: elev::Elevator,
    initial_state: ElevatorState,
    initial_floor: u8,
    mut rx_elevator_manager: mpsc::Receiver<MsgToElevatorManager>,
    tx_call_manager: mpsc::Sender<MsgToCallManager>,
    tx_elevator_manager: mpsc::Sender<MsgToElevatorManager>,
    tx_world_manager: mpsc::Sender<MsgToWorldView>,
) {
    let mut elevator = Elevator::new(driver.clone(), initial_state, initial_floor);

    while let Some(msg) = rx_elevator_manager.recv().await {
        match msg {
            MsgToElevatorManager::AtFloor(floor) => {
                println!("AtFloor: {}", floor);
                elevator.current_floor = floor;
                elevator.driver.floor_indicator(floor);

                if elevator.calls.is_empty() {
                    elevator.driver.motor_direction(elev::DIRN_STOP);
                    elevator.state = ElevatorState::Idle;
                    elevator.direction = Direction::Stop;
                }

                if elevator.state == ElevatorState::Moving && elevator.should_serve_here() {
                    elevator.driver.motor_direction(elev::DIRN_STOP);
                    elevator.serve_current_floor(&tx_call_manager).await;
                    elevator.open_door(tx_elevator_manager.clone());

                    elevator.send_status_update(&tx_world_manager).await;
                }

                if floor == 0 || floor == 3 {
                    elevator.driver.motor_direction(elev::DIRN_STOP);
                }
            }
            MsgToElevatorManager::ActiveCalls(calls) => {
                if elevator.is_obstructed {
                    continue;
                }

                elevator.calls = calls;
                if elevator.state == ElevatorState::Idle {
                    if elevator.should_serve_here() {
                        if elevator.direction == Direction::Stop {
                            elevator.direction = elevator.next_direction();
                        }

                        elevator.serve_current_floor(&tx_call_manager).await;
                        elevator.open_door(tx_elevator_manager.clone());
                    } else {
                        elevator.serve_next_action();
                    }
                    elevator.send_status_update(&tx_world_manager).await;
                }
            }
            MsgToElevatorManager::DoorClosed => {
                if elevator.is_obstructed {
                    elevator.open_door(tx_elevator_manager.clone());
                    continue;
                }

                elevator.driver.door_light(false);
                if elevator.should_serve_here() {
                    elevator.serve_current_floor(&tx_call_manager).await;
                    elevator.open_door(tx_elevator_manager.clone());
                } else {
                    elevator.serve_next_action();
                }
                elevator.send_status_update(&tx_world_manager).await;
            }
            MsgToElevatorManager::Obstruction(is_obstructed) => {
                elevator.is_obstructed = is_obstructed;
                println!("Is obstructed: {}", is_obstructed);
                if is_obstructed && elevator.state == ElevatorState::DoorOpen {
                        elevator.open_door(tx_elevator_manager.clone());
                }

                elevator.send_status_update(&tx_world_manager).await;
            }
        }
    }
}
