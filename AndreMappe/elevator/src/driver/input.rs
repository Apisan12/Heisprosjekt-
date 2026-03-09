use std::{thread, time::Duration};
use tokio::sync::mpsc;

use driver_rust::elevio::elev::Elevator;

use crate::messages::{Call, CallId, MsgToElevatorManager, MsgToWorldView, NodeId};

/// Spawns the hardware polling thread.
///
/// Responsibilities:
/// - Poll call buttons -> Sends to world_manager
/// - Poll floor sensor -> Sends to elevator_manager
/// - Poll stop button -> Sends to elevator_manager
/// - Poll obstruction switch -> Sends to elevator_manager
pub fn spawn_input_thread(
elev_id: NodeId,
elevator: Elevator,
tx_world_view_msg: mpsc::Sender<MsgToWorldView>,
tx_fsm_msg: mpsc::Sender<MsgToElevatorManager>,
period: Duration,
) {
thread::spawn(move || {
// Previous state tracking
let mut prev_buttons = vec![[false; 3]; elevator.num_floors as usize];
let mut prev_floor: Option<u8> = None;
// let mut prev_stop = false;
let mut prev_obstruction = false;
let mut seq: u64 = 0;

    loop {
        // --- Call buttons ---
        for floor in 0..elevator.num_floors {
            for call in 0..3 {
                let pressed = elevator.call_button(floor, call);

                if pressed && prev_buttons[floor as usize][call as usize] != pressed {
                    let call_id = CallId{
                        elev_id: elev_id,
                        seq: seq,
                    };
                    seq = seq.wrapping_add(1);
                    let call = Call {
                        id: call_id,
                        floor: floor,
                        call_type: call,
                    };
                    let _ = tx_world_view_msg.blocking_send(MsgToWorldView::AddCall(call));
                    println!("Call sent to WorldView: {}", call)
                }

                prev_buttons[floor as usize][call as usize] = pressed;
            }
        }

        // --- Floor sensor ---
        let floor = elevator.floor_sensor();

        if floor != prev_floor {
            if let Some(f) = floor {
                let _ = tx_fsm_msg.blocking_send(MsgToElevatorManager::AtFloor(f));
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
        let obstruction = elevator.obstruction();

        if obstruction != prev_obstruction {
            let _ = tx_fsm_msg.blocking_send(MsgToElevatorManager::Obstruction(obstruction));
            prev_obstruction = obstruction;
        }

        thread::sleep(period);
    }
});

}
