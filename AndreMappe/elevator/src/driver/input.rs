//! Hardware input manager.
//!
//! Polls the elevator hardware driver and forwards detected events to the
//! system managers. Button presses are sent to the `world_manager`, while
//! floor arrivals and obstruction changes are sent to the `elevator_manager`.
//!
//! The hardware is polled periodically using the interval defined by
//! `ELEV_POLL`. State changes are detected to avoid sending duplicate events.

use std::thread;
use tokio::sync::mpsc;
use driver_rust::elevio::elev::{self, CAB, HALL_DOWN, HALL_UP};

use crate::{
    config::{BOTTOM_FLOOR, ELEV_POLL, TOP_FLOOR},
    messages::{Call, CallId, ElevatorId, MsgToElevatorManager, MsgToWorldManager},
};

/// Thread for polling the hardware.
///
/// Responsibilities:
/// - Poll call buttons -> Sends to world_manager
/// - Poll floor sensor -> Sends to elevator_manager
/// - Poll stop button -> Sends to elevator_manager
/// - Poll obstruction switch -> Sends to elevator_manager
pub fn input_manager(
    elevator_id: ElevatorId,
    driver: elev::Elevator,
    tx_world_manager: mpsc::Sender<MsgToWorldManager>,
    tx_elevator_manager: mpsc::Sender<MsgToElevatorManager>,
) {
    thread::spawn(move || {
        // Previous state tracking
        let mut prev_buttons = vec![[false; 3]; driver.num_floors as usize];
        let mut prev_floor: Option<u8> = None;
        // let mut prev_stop = false;
        let mut prev_obstruction = false;

        // Sequence to identify calls.
        // Using u64 means the counter can take 18 quintillion increments before it wraps.
        let mut seq: u64 = 0;

        loop {
            // --- Call buttons ---
            for floor in BOTTOM_FLOOR..TOP_FLOOR {
                for call in [HALL_UP, HALL_DOWN, CAB] {
                    let pressed = driver.call_button(floor, call);

                    if pressed && prev_buttons[floor as usize][call as usize] != pressed {

                        // Converts the call into the Call struct before sending it to the world_manager.
                        let call_id = CallId {
                            elev_id: elevator_id,
                            seq: seq,
                        };

                        seq = seq.wrapping_add(1);
                        let call = Call {
                            id: call_id,
                            floor: floor,
                            call_type: call,
                        };
                        let _ = tx_world_manager.blocking_send(MsgToWorldManager::AddCall(call));
                    }

                    prev_buttons[floor as usize][call as usize] = pressed;
                }
            }

            // --- Floor sensor ---
            let floor = driver.floor_sensor();

            if floor != prev_floor {
                if let Some(f) = floor {
                    let _ = tx_elevator_manager.blocking_send(MsgToElevatorManager::AtFloor(f));
                }
                prev_floor = floor;
            }

            // --- Stop button ---
            // let stop = elevator.stop_button();

            // if stop != prev_stop {
            //     let _ = tx.send(DriverEvent::StopButton(stop));
            //     prev_stop = stop;
            // }

            // --- Obstruction ---
            let obstruction = driver.obstruction();

            if obstruction != prev_obstruction {
                let _ = tx_elevator_manager
                    .blocking_send(MsgToElevatorManager::Obstruction(obstruction));
                prev_obstruction = obstruction;
            }

            thread::sleep(ELEV_POLL);
        }
    });
}
