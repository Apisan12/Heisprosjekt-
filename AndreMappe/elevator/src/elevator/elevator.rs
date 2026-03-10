//! Elevator manager.
//!
//! This module implements the **local elevator manager**.
//!
//! The manager maintains the elevator state machine and reacts to
//! events received from other components through message channels.
//!
//! Responsibilities:
//! - Track elevator state and position
//! - Decide movement direction based on assigned calls
//! - Serve calls at floors
//! - Control door timing
//! - Handle obstruction events
//! - Send status updates to the world view
//!
//! The elevator manager communicates with:
//!
//! - `call_manager` – reports when calls have been served
//! - `world_manager` – sends updates about elevator state
//! - `Elevator driver` – hardware interface for motor, doors, and sensors

use crate::config::{BOTTOM_FLOOR, TOP_FLOOR};
use crate::messages::{Call, MsgToCallManager, MsgToElevatorManager, MsgToWorldManager};
use driver_rust::elevio::elev::{self, CAB, HALL_DOWN, HALL_UP};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tokio::sync::mpsc;
use tokio::time::{Duration, sleep};

/// Internal state of the elevator
///
/// This represent the physical state of the elevator.
#[derive(PartialEq, Eq)]
pub enum ElevatorState {
    /// Elevator is stationary and waiting for calls.
    Idle,

    /// Elevator motor is running and the elevator is moving.
    Moving,

    /// Elevator doors are open at a floor.
    DoorOpen,

    /// When the elevator has stopped because of the stop button. (NOT IMPLEMENTED)
    _Stopped,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Behaviour {
    Idle,
    Moving,
    DoorOpen,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    Up,
    Down,
    Stop,
}

impl ElevatorState {
    /// Converts the internal elevator state to a [`Behaviour`]
    pub fn behaviour(&self) -> Behaviour {
        match self {
            ElevatorState::Idle => Behaviour::Idle,
            ElevatorState::Moving => Behaviour::Moving,
            ElevatorState::DoorOpen => Behaviour::DoorOpen,
            ElevatorState::_Stopped => Behaviour::Idle,
        }
    }
}

/// Status information about the local elevator.
///
/// This struct is sent to the `WorldView` every time there is a change
/// in one of the fields. This is to make sure everything using the
///  `WorldView` to do decisions has the current status.
#[derive(Debug, Clone)]
pub struct LocalElevatorStatus {
    pub floor: u8,
    pub direction: Direction,
    pub behaviour: Behaviour,
    /// Indicates whether the obstruction sensor is active.
    pub is_obstructed: bool,
}

impl LocalElevatorStatus {
    /// Creates a new elevator status message.
    pub fn new(floor: u8, direction: Direction, behaviour: Behaviour, is_obstructed: bool) -> Self {
        Self {
            floor,
            direction,
            behaviour,
            is_obstructed,
        }
    }
}

/// Internal representation of the elevator.
///
/// This struct encapsulates the elevator.
struct Elevator {
    /// Hardware driver for interacting with the elevator output:
    /// - Motor direction
    /// - Door, floor and button lights
    driver: elev::Elevator,
    /// Current State of the elevator.
    state: ElevatorState,
    current_floor: u8,
    /// Current movement direction.
    direction: Direction,
    /// Set of calls assigned to this elevator.
    /// This includes both the cab and hall calls.
    calls: HashSet<Call>,
    /// True if obstruction sensor is active.
    is_obstructed: bool,
}

impl Elevator {
    /// Creates a new elevator instance
    ///
    /// The initial state and floor are determined during system startup.
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
    /// Returns true if there are calls above the current floor.
    fn has_calls_above(&self) -> bool {
        self.calls
            .iter()
            .any(|call| call.floor > self.current_floor)
    }
    /// Returns true if there are calls below the current floor.
    fn has_calls_below(&self) -> bool {
        self.calls
            .iter()
            .any(|call| call.floor < self.current_floor)
    }

    /// Determines the next direction the elevator should move.
    ///
    /// The algorithm prioritizes continuing in the current direction
    /// if there are remaining calls ahead.
    ///
    /// If no calls remain in that direction, the elevator reverses
    /// direction or stops if no more calls exist.
    fn next_direction(&self) -> Direction {
        if self.direction != Direction::Down && self.has_calls_above() {
            Direction::Up
        } else if self.direction != Direction::Up && self.has_calls_below() {
            Direction::Down
        } else {
            Direction::Stop
        }
    }

    /// Determines which direction should be served at the current floor
    ///
    /// This is used when deciding whether hall calls should be served.
    fn service_direction(&self) -> Direction {
        if self.direction == Direction::Up && self.has_calls_above() {
            return Direction::Up;
        }

        if self.direction == Direction::Down && self.has_calls_below() {
            return Direction::Down;
        }

        // If no more calls in the current movement direction, checks if
        // there is any hall calls on this floor.
        let has_up = self
            .calls
            .iter()
            .any(|call| call.floor == self.current_floor && call.call_type == HALL_UP);

        let has_down = self
            .calls
            .iter()
            .any(|call| call.floor == self.current_floor && call.call_type == HALL_DOWN);

        // Returns the direction of the hall call or stop if there is no
        // hall calls on this floor.
        if has_up {
            Direction::Up
        } else if has_down {
            Direction::Down
        } else {
            Direction::Stop
        }
    }

    /// Determines whether the elevator should serve calls
    /// at the current floor.
    ///
    /// Rules:
    /// - Cab calls are always served.
    /// - Hall calls are served only if they match the service direction.
    ///
    /// Returns true if there is a call to serve.
    fn should_serve_here(&self) -> bool {
        let service_direction = self.service_direction();

        self.calls.iter().any(|call| {
            call.floor == self.current_floor
                && match call.call_type {
                    CAB => true,

                    HALL_UP => service_direction == Direction::Up,

                    HALL_DOWN => service_direction == Direction::Down,

                    _ => false,
                }
        })
    }

    /// Returns the set of calls that will be served at the current floor.
    ///
    /// Rules:
    /// - Cab calls are always served.
    /// - Hall calls are served only if they match the service direction.
    fn served_calls(&self) -> HashSet<Call> {
        let service_direction = self.service_direction();

        self.calls
            .iter()
            .filter(|call| {
                call.floor == self.current_floor
                    && match call.call_type {
                        CAB => true,

                        HALL_UP => service_direction == Direction::Up,

                        HALL_DOWN => service_direction == Direction::Down,

                        _ => false,
                    }
            })
            .cloned()
            .collect()
    }

    /// Serves calls at the current floor
    /// This removes the served calls from this elevator,
    /// and notifies the call manager that the call is served.
    ///
    /// Takes in the call manager channel as parameter.
    async fn serve_current_floor(&mut self, tx_call_manager: &mpsc::Sender<MsgToCallManager>) {
        let served = self.served_calls();

        for call in served {
            self.calls.remove(&call);
            let _ = tx_call_manager
                .send(MsgToCallManager::ServedCall(call))
                .await;
        }
    }

    /// Opens the elevator door and starts the door timer.
    ///
    /// The door remains open for 3 second before a
    /// `DoorClosed` message is sent.
    ///
    /// Takes in the elevator manager channel as parameter.
    fn open_door(&mut self, tx_elevator_manager: mpsc::Sender<MsgToElevatorManager>) {
        self.driver.door_light(true);
        self.state = ElevatorState::DoorOpen;
        self.direction = Direction::Stop;

        tokio::spawn(async move {
            sleep(Duration::from_secs(3)).await;
            let _ = tx_elevator_manager
                .send(MsgToElevatorManager::DoorClosed)
                .await;
        });
    }

    /// Determines and executes the next movement action.
    ///
    /// The elevator will either:
    /// - Move up
    /// - Move down
    /// - Remain idle
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

    /// Sends the current elevator status to the `WorldView`.
    ///
    /// Takes in the world manager channel as parameter.
    async fn send_new_status(&self, tx_world_manager: &mpsc::Sender<MsgToWorldManager>) {
        let status = LocalElevatorStatus::new(
            self.current_floor,
            self.direction,
            self.state.behaviour(),
            self.is_obstructed,
        );

        let _ = tx_world_manager
            .send(MsgToWorldManager::NewLocalElevStatus(status))
            .await;
    }
}

/// Asynchronous task controlling the elevator.
///
/// This function acts as the manager for the elevator.
/// It listens for messages from other system modules and updates
/// the elevator state accordingly.
///
/// # Message Types
///
/// `AtFloor`
/// : Triggered by the floor sensor when the elevator reaches a floor.
/// Received from the input thread.
///
/// `ActiveCalls`
/// : Updated list of calls assigned to this elevator.
/// Received from the call manager.
///
/// `DoorClosed`
/// : Triggered after the door timer expires
/// Received from the elevator manager.
///
/// `Obstruction`
/// : Triggered when the obstruction sensor changes state.
/// Received from the input thread.
///
/// The manager sends messages to:
///
/// - `CallManager` - Sends served calls.
/// - `WorldView` - Sends new elevator statuses.
/// - `Driver` - Controls motor, door light and floor indicator.
pub async fn elevator_manager(
    driver: elev::Elevator,
    initial_state: ElevatorState,
    initial_floor: u8,
    mut rx_elevator_manager: mpsc::Receiver<MsgToElevatorManager>,
    tx_call_manager: mpsc::Sender<MsgToCallManager>,
    tx_elevator_manager: mpsc::Sender<MsgToElevatorManager>,
    tx_world_manager: mpsc::Sender<MsgToWorldManager>,
) {
    let mut elevator = Elevator::new(driver.clone(), initial_state, initial_floor);

    while let Some(msg) = rx_elevator_manager.recv().await {
        match msg {
            MsgToElevatorManager::AtFloor(floor) => {
                println!("At floor: {}", floor);
                elevator.current_floor = floor;
                elevator.driver.floor_indicator(floor);

                // Stops the elevator at a floor if it has been unassigned a call it was going towards.
                if elevator.calls.is_empty() {
                    elevator.driver.motor_direction(elev::DIRN_STOP);
                    elevator.state = ElevatorState::Idle;
                    elevator.direction = Direction::Stop;
                }

                // Stops if it has calls to server at this floor.
                if elevator.should_serve_here() {
                    elevator.driver.motor_direction(elev::DIRN_STOP);
                    elevator.serve_current_floor(&tx_call_manager).await;
                    elevator.open_door(tx_elevator_manager.clone());
                }

                // Stops the elevator at the bottom or top floor, used as a defensive
                // mechanism since there is no circumstance the elevator should move
                // past these floors.
                if floor == BOTTOM_FLOOR || floor == TOP_FLOOR {
                    elevator.driver.motor_direction(elev::DIRN_STOP);
                    elevator.state = ElevatorState::Idle;
                    elevator.direction = Direction::Stop;
                }

                elevator.send_new_status(&tx_world_manager).await;
            }
            MsgToElevatorManager::ActiveCalls(calls) => {
                elevator.calls = calls;

                if elevator.is_obstructed {
                    continue;
                }

                // If the elevator is Idle, check if there is new calls to serve
                if elevator.state == ElevatorState::Idle {
                    if elevator.should_serve_here() {
                        elevator.direction = elevator.next_direction();
                        elevator.serve_current_floor(&tx_call_manager).await;
                        elevator.open_door(tx_elevator_manager.clone());
                    } else {
                        elevator.serve_next_action();
                    }
                    elevator.send_new_status(&tx_world_manager).await;
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
                elevator.send_new_status(&tx_world_manager).await;
            }
            MsgToElevatorManager::Obstruction(is_obstructed) => {
                elevator.is_obstructed = is_obstructed;
                println!("Is obstructed: {}", is_obstructed);
                if is_obstructed && elevator.state == ElevatorState::DoorOpen {
                    elevator.open_door(tx_elevator_manager.clone());
                }
                elevator.send_new_status(&tx_world_manager).await;
            }
        }
    }
}
