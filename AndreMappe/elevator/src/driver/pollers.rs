use std::thread::spawn;
use std::time::Duration;
use crossbeam_channel as cbc;
use driver_rust::elevio::elev as e;
use driver_rust::elevio::poll;

pub struct PollReceivers {
    pub call_button: cbc::Receiver<poll::CallButton>,
    pub floor_sensor: cbc::Receiver<u8>,
    pub stop_button: cbc::Receiver<bool>,
    pub obstruction: cbc::Receiver<bool>,
}

pub fn spawn_input_pollers(
    elevator: e::Elevator,
    poll_period: Duration,
) -> PollReceivers {
    let (call_button_tx, call_button_rx) = cbc::unbounded();
    {
        let elevator = elevator.clone();
        spawn(move || poll::call_buttons(elevator, call_button_tx, poll_period));
    }

    let (floor_sensor_tx, floor_sensor_rx) = cbc::unbounded();
    {
        let elevator = elevator.clone();
        spawn(move || poll::floor_sensor(elevator, floor_sensor_tx, poll_period));
    }

    let (stop_button_tx, stop_button_rx) = cbc::unbounded();
    {
        let elevator = elevator.clone();
        spawn(move || poll::stop_button(elevator, stop_button_tx, poll_period));
    }

    let (obstruction_tx, obstruction_rx) = cbc::unbounded();
    {
        let elevator = elevator.clone();
        spawn(move || poll::obstruction(elevator, obstruction_tx, poll_period));
    }

    PollReceivers {
        call_button: call_button_rx,
        floor_sensor: floor_sensor_rx,
        stop_button: stop_button_rx,
        obstruction: obstruction_rx,
    }

}