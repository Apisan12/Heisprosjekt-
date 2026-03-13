//! Elevator manager.
//!
//! This module implements the **local elevator manager**.
//!
//! The manager maintains the elevator state machine and reacts to
//! messages received from other components through tokio channels.
//!
//! Responsibilities:
//! - Track elevator state and position
//! - Decide movement direction based on assigned calls
//! - Serve calls at floors
//! - Control door timing
//! - Detect travel timeouts between floors
//! - Handle obstruction events
//! - Send status updates to the world view
//!
//! The elevator manager communicates with:
//!
//! - `world_manager` – sends updates about elevator state
//! - `driver` – hardware interface for motor, doors, and sensors

use crate::config::{BOTTOM_FLOOR, DOOR_TIMEOUT, TOP_FLOOR, TRAVEL_TIMEOUT};
use crate::messages::{Call, MsgToElevatorManager, MsgToWorldManager, LocalElevatorStatus};
use driver_rust::elevio::elev::{self, CAB, HALL_DOWN, HALL_UP};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tokio::sync::mpsc;
use tokio::time::{Duration, Instant, sleep_until};

/// External representation of the elevator behaviour.
///
/// This enum is used when reporting the elevator state to the
/// `world_manager`. It is a simplified representation of
/// [`ElevatorState`] that only exposes the behaviour relevant
/// for the other system components.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Behaviour {
    Idle,
    Moving,
    DoorOpen,
}

/// Direction of travel for the elevator.
///
/// This is used both internally by the elevator manager and when
/// reporting the elevator state to the `world_manager`.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    Up,
    Down,
    Stop,
}

/// Internal state of the elevator
///
/// Represent the physical state of the elevator.
#[derive(PartialEq, Eq)]
enum ElevatorState {
    /// Elevator is stationary and waiting for calls.
    Idle,

    /// Elevator motor is running and the elevator is moving.
    Moving,

    /// Elevator doors are open at a floor.
    DoorOpen,

}

impl ElevatorState {
    /// Converts the internal elevator state to a [`Behaviour`]
    pub fn behaviour(&self) -> Behaviour {
        match self {
            ElevatorState::Idle => Behaviour::Idle,
            ElevatorState::Moving => Behaviour::Moving,
            ElevatorState::DoorOpen => Behaviour::DoorOpen,
        }
    }
}


/// Internal representation of the elevator.
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
    /// True if the elevator used to long to travel between floors.
    /// This indicates a temporary motor or movemement fault and prevents
    /// the elevator from receiving new assignments.
    travel_timeout: bool,
}

impl Elevator {
    /// Creates a new elevator instance
    ///
    /// The initial state and floor are determined during system startup.
    fn new(driver: elev::Elevator, initial_floor: u8) -> Self {
        Self {
            driver: driver.clone(),
            state: ElevatorState::Idle,
            current_floor: initial_floor,
            direction: Direction::Stop,
            calls: HashSet::new(),
            is_obstructed: driver.obstruction(),
            travel_timeout: false,
        }
    }

    /// Stops the elevator motor and resets the movement state.
    ///
    /// This function updates both the motor direction and the
    /// internal state machine to reflect that the elevator is idle.
    fn stop(&mut self) {
        self.driver.motor_direction(elev::DIRN_STOP);
        self.state = ElevatorState::Idle;
    }

    /// Determines and executes the next movement action.
    ///
    /// Based on the currently assigned calls, this function decides
    /// whether the elevator should move up, move down or remain idle.
    /// The motor direction and internal state machine are updated
    /// accordingly.
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

    /// Serves calls at the current floor.
    ///
    /// The calls that are served are removed from the elevator's
    /// internal call set and reported to the call manager so the
    /// global system state can be updated.
    async fn serve_current_floor(&mut self, tx_world_manager: &mpsc::Sender<MsgToWorldManager>) {
        let served = self.served_calls();

        for call in served {
            self.calls.remove(&call);
            let _ = tx_world_manager
                .send(MsgToWorldManager::ServedCall(call))
                .await;
        }
    }

    /// Determines the next direction using a SCAN-like scheduling strategy.
    ///
    /// The elevator continues in its current direction while there
    /// are remaining calls ahead. If no calls remain in that direction,
    /// the elevator reverses direction or stops if no calls exist.
    fn next_direction(&self) -> Direction {
        if self.direction != Direction::Down && self.has_calls_above() {
            Direction::Up
        } else if self.direction != Direction::Up && self.has_calls_below() {
            Direction::Down
        } else {
            Direction::Stop
        }
    }

    /// Determines the direction that should be served at the current floor.
    ///
    /// This is used when deciding whether hall calls should be served
    /// while the elevator is stopping at a floor.
    ///
    /// If the elevator still has calls ahead in its current travel
    /// direction, that direction is preferred. Otherwise the function
    /// checks if there are hall calls at the current floor and returns
    /// their direction.
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
    /// Uses the same rules as [`Self::should_serve_here`].
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

    /// Opens the elevator door.
    fn open_door(&mut self) {
        self.driver.door_light(true);
        self.state = ElevatorState::DoorOpen;
        self.direction = self.next_direction();
    }

    /// Sends the current elevator status to the `WorldView`.
    ///
    /// This keeps the distributed system synchronized with the
    /// latest state of the local elevator.
    async fn send_new_status(&self, tx_world_manager: &mpsc::Sender<MsgToWorldManager>) {
        let status = LocalElevatorStatus::new(
            self.current_floor,
            self.direction,
            self.state.behaviour(),
            self.is_obstructed || self.travel_timeout,
        );

        let _ = tx_world_manager
            .send(MsgToWorldManager::NewLocalElevatorStatus(status))
            .await;
    }
}

/// Simple watchdog timer used for door and travel timeouts.
/// 
/// The watchdog is started with a deadline and can be awaited
/// inside `tokio::select!`. When inactive, the timer is disabled.
/// 
/// Used for:
/// - Door open timeout
/// - Elevator travel time between floors
struct Watchdog {
    deadline: Option<Instant>,
}

impl Watchdog {
    fn new() -> Self {
        Self { deadline: None }
    }

    /// Starts the watchdog with the given time duration.
    fn start(&mut self, duration: Duration) {
        self.deadline = Some(Instant::now() + duration);
    }

    /// Stops the watchdog and disables the timer.
    fn stop(&mut self) {
        self.deadline = None;
    }

    /// Future that resolves when the deadline expires.
    async fn wait(&self) {
        if let Some(deadline) = self.deadline {
            sleep_until(deadline).await;
        }
    }

    /// Returns tue if the watchdog is currently active.
    fn active(&self) -> bool {
        self.deadline.is_some()
    }
}

/// Asynchronous task controlling the local elevator.
///
/// The elevator manager maintains the elevator state machine and
/// reacts to messages received through a Tokio channel.
///
/// It handles floor arrivals, call assignments, and obstruction
/// events, while also monitoring door and travel timeouts using
/// watchdog timers.
///
/// After state changes, the manager sends updated elevator status
/// messages to the `world_manager`.
pub async fn elevator_manager(
    driver: elev::Elevator,
    initial_floor: u8,
    mut rx_elevator_manager: mpsc::Receiver<MsgToElevatorManager>,
    tx_world_manager: mpsc::Sender<MsgToWorldManager>,
) {
    let mut elevator = Elevator::new(driver.clone(), initial_floor);
    let mut door_timer = Watchdog::new();
    let mut travel_timer =  Watchdog::new();

    loop {
        tokio::select! {

            Some(msg) = rx_elevator_manager.recv() => {
                match msg {
                    // Triggered when the floor sensor detects a new floor.
                    MsgToElevatorManager::AtFloor(floor) => {
                        println!("At floor: {}", floor);
                        elevator.current_floor = floor;
                        elevator.driver.floor_indicator(floor);
                        travel_timer.stop();
                        elevator.travel_timeout = false;

                        // Stops the elevator at a floor if it has been unassigned a call it was going towards.
                        // This can happen if a new elevator joins the network that is closer to the call.
                        if elevator.calls.is_empty() {
                            elevator.stop();
                        }

                        else if elevator.should_serve_here() {
                            elevator.stop();
                            elevator.serve_current_floor(&tx_world_manager).await;
                            elevator.open_door();
                            door_timer.start(DOOR_TIMEOUT);
                        }

                        // Stops the elevator at the bottom or top floor, used as a defensive
                        // mechanism since there is no circumstance the elevator should move
                        // past these floors.
                        else if floor == BOTTOM_FLOOR || floor == TOP_FLOOR {
                            elevator.stop();
                        }

                        elevator.send_new_status(&tx_world_manager).await;
                    }
                    // Updated call assignments from the call manager.
                    MsgToElevatorManager::ActiveCalls(calls) => {
                        elevator.calls = calls;

                        if elevator.is_obstructed {
                            continue;
                        }

                        // Set intended travel direction before deciding which hall calls to serve
                        elevator.direction = elevator.next_direction();

                        if elevator.state == ElevatorState::Idle {
                            if elevator.should_serve_here() {
                                elevator.serve_current_floor(&tx_world_manager).await;
                                elevator.open_door();
                                door_timer.start(DOOR_TIMEOUT);
                            } else {
                                elevator.serve_next_action();
                                if elevator.state == ElevatorState::Moving {
                                    travel_timer.start(TRAVEL_TIMEOUT);
                                }
                            }
                            elevator.send_new_status(&tx_world_manager).await;
                        }
                    }

                    // Obstruction sensor changed state.
                    MsgToElevatorManager::Obstruction(is_obstructed) => {
                        elevator.is_obstructed = is_obstructed;
                        println!("Is obstructed: {}", is_obstructed);
                        if is_obstructed && elevator.state == ElevatorState::DoorOpen {
                            elevator.open_door();
                            door_timer.start(DOOR_TIMEOUT);
                        }
                        elevator.send_new_status(&tx_world_manager).await;
                    }
                }
            }

            // Door timer expired, elevator door is closed.
            _ = door_timer.wait(), if door_timer.active() => {

                door_timer.stop();

                if elevator.is_obstructed {
                    elevator.open_door();
                    door_timer.start(DOOR_TIMEOUT);
                    continue;
                }
                // Update intended travel direction before deciding whether to serve or depart.
                elevator.direction = elevator.next_direction();
                elevator.driver.door_light(false);

                if elevator.should_serve_here() {
                    elevator.serve_current_floor(&tx_world_manager).await;
                    elevator.open_door();
                    door_timer.start(DOOR_TIMEOUT);
                } else {
                    elevator.serve_next_action();
                    if elevator.state == ElevatorState::Moving {
                        travel_timer.start(TRAVEL_TIMEOUT);
                    }
                }
                elevator.send_new_status(&tx_world_manager).await;
            }

            // Travel timer expired, elevator did not reach a floor in time.
            _ = travel_timer.wait(), if travel_timer.active() => {
                println!("Travel timeout detected");

                elevator.travel_timeout = true;
                travel_timer.stop();

                elevator.send_new_status(&tx_world_manager).await;
            }
        }
    }
}

