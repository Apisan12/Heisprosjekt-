use std::collections::HashSet;

use crate::messages::{
    Behaviour, Call, Direction, ElevatorStatus, MsgToCallManager, MsgToElevatorManager, MsgToWorldView, NodeId,
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
    state: ElevatorState,
    current_floor: u8,
    direction: Direction,
    previous_direction: Direction,
    calls: HashSet<Call>,
}

impl Elevator {
    fn new(initial_state: ElevatorState, initial_floor: u8) -> Self {
        Self {
            state: initial_state,
            current_floor: initial_floor,
            direction: Direction::Stop,
            previous_direction: Direction::Stop,
            calls: HashSet::new(),
        }
    }

    fn any_calls_at_floor(&self, floor: u8) -> bool {
        self.calls.iter().any(|call| call.floor == floor)
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

    fn should_stop(&self) -> bool {
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

    fn open_door(&self, tx: mpsc::Sender<MsgToElevatorManager>) {
        tokio::spawn(async move {
            sleep(Duration::from_secs(3)).await;
            let _ = tx.send(MsgToElevatorManager::DoorTimeout).await;
        });
    }

    async fn send_status_update(&self, tx_world_view_msg: &mpsc::Sender<MsgToWorldView>) {
        let status = LocalElevatorStatus::new(self.current_floor, self.direction, self.state.behaviour());

        let _ = tx_world_view_msg
            .send(MsgToWorldView::UpdateLocalElevStatus(status))
            .await;
    }

}

pub async fn elevator_manager(
    driver: elev::Elevator,
    initial_state: ElevatorState,
    current_floor: u8,
    mut rx_elevator_manager: mpsc::Receiver<MsgToElevatorManager>,
    tx_call_manager: mpsc::Sender<MsgToCallManager>,
    tx_elevator_manager: mpsc::Sender<MsgToElevatorManager>,
    tx_world_manager: mpsc::Sender<MsgToWorldView>,
) {
    let elevator = Elevator::new(initial_state, current_floor);
    // Tar imot events
    while let Some(msg) = rx_elevator_manager.recv().await {
        match msg {
            MsgToElevatorManager::AtFloor(floor) => {
                println!("AtFloor: {}", floor);
                elevator.current_floor = floor;
                driver.floor_indicator(floor);

                if elevator.state == ElevatorState::Moving && elevator.should_stop() {
                    driver.motor_direction(elev::DIRN_STOP);
                    elevator.previous_direction = elevator.direction;
                    elevator.direction = Direction::Stop;

                    let served = elevator.served_calls();
                    for call in served {
                        println!("Call served: {}", call);
                        elevator.calls.remove(&call);
                        let _ = tx_call_manager
                            .send(MsgToCallManager::ServedCall(call))
                            .await;
                    }

                    driver.door_light(true);
                    elevator.open_door(tx_elevator_manager.clone());
                    elevator.state == ElevatorState::DoorOpen;

                    elevator.send_status_update(&tx_world_manager).await;
                }
            }
            MsgToElevatorManager::ActiveCalls(calls) => {
                elevator.calls = calls;
            },
            MsgToElevatorManager::DoorTimeout => {
                driver.door_light(false);

                let dir = choose_next_direction(&calls, current_floor, state.direction());

                match dir {
                    Direction::Up => {
                        driver.motor_direction(elev::DIRN_UP);
                        state = ElevatorState::Moving(Direction::Up);
                    }

                    Direction::Down => {
                        driver.motor_direction(elev::DIRN_DOWN);
                        state = ElevatorState::Moving(Direction::Down);
                    }

                    Direction::Stop => {
                        state = ElevatorState::Idle;
                    }
                }

                send_elevator_status_update(current_floor, &state, &tx_world_manager).await;
            }
        }
    }
}

fn choose_next_direction(
    calls: &HashSet<Call>,
    current_floor: u8,
    current_dir: Direction,
) -> Direction {
    match current_dir {
        Direction::Up => {
            if calls.iter().any(|c| c.floor > current_floor) {
                Direction::Up
            } else if calls.iter().any(|c| c.floor < current_floor) {
                Direction::Down
            } else {
                Direction::Stop
            }
        }

        Direction::Down => {
            if calls.iter().any(|c| c.floor < current_floor) {
                Direction::Down
            } else if calls.iter().any(|c| c.floor > current_floor) {
                Direction::Up
            } else {
                Direction::Stop
            }
        }

        Direction::Stop => {
            if calls.iter().any(|c| c.floor > current_floor) {
                Direction::Up
            } else if calls.iter().any(|c| c.floor < current_floor) {
                Direction::Down
            } else {
                Direction::Stop
            }
        }
    }
}
