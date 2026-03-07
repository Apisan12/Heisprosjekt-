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
}

impl LocalElevatorStatus {
    pub fn new(floor: u8, direction: Direction, behaviour: Behaviour) -> Self {
        Self {
            floor,
            direction,
            behaviour,
        }
    }
}

struct Elevator {
    driver: elev::Elevator,
    state: ElevatorState,
    current_floor: u8,
    direction: Direction,
    calls: HashSet<Call>,
}

impl Elevator {
    fn new(driver: elev::Elevator, initial_state: ElevatorState, initial_floor: u8) -> Self {
        Self {
            driver: driver,
            state: initial_state,
            current_floor: initial_floor,
            direction: Direction::Stop,
            calls: HashSet::new(),
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

    fn should_serve_here(&self) -> bool {
        self.calls.iter().any(|call| {
            call.floor == self.current_floor
                && match call.call_type {
                    CAB => true,

                    HALL_UP => self.direction == Direction::Up || !self.has_calls_above(),

                    HALL_DOWN => self.direction == Direction::Down || !self.has_calls_below(),

                    _ => false,
                }
        })
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

    fn served_calls(&self) -> HashSet<Call> {
        self.calls
            .iter()
            .filter(|call| {
                call.floor == self.current_floor
                    && match call.call_type {
                        CAB => true,

                        HALL_UP => self.direction == Direction::Up || !self.has_calls_above(),

                        HALL_DOWN => self.direction == Direction::Down || !self.has_calls_below(),

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
            LocalElevatorStatus::new(self.current_floor, self.direction, self.state.behaviour());

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

                if elevator.state == ElevatorState::Moving && elevator.should_serve_here() {
                    elevator.driver.motor_direction(elev::DIRN_STOP);
                    elevator.open_door(tx_elevator_manager.clone());
                    elevator.serve_current_floor(&tx_call_manager).await;
                    elevator.send_status_update(&tx_world_manager).await;
                }
            }
            MsgToElevatorManager::ActiveCalls(calls) => {
                elevator.calls = calls;
                if elevator.state == ElevatorState::Idle {
                    if elevator.should_serve_here() {
                        elevator.open_door(tx_elevator_manager.clone());
                        elevator.serve_current_floor(&tx_call_manager).await;
                    } else {
                        elevator.serve_next_action();
                    }
                    elevator.send_status_update(&tx_world_manager).await;
                }
            }
            MsgToElevatorManager::DoorClosed => {
                elevator.driver.door_light(false);
                if elevator.should_serve_here() {
                    elevator.open_door(tx_elevator_manager.clone());
                    elevator.serve_current_floor(&tx_call_manager).await;
                } else {
                    elevator.serve_next_action();
                }
                elevator.send_status_update(&tx_world_manager).await;
            }
        }
    }
}
