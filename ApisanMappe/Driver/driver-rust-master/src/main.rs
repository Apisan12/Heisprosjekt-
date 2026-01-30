use std::thread::spawn;
use std::time::{Duration, Instant};

use crossbeam_channel as cbc;

use driver_rust::elevio;
use driver_rust::elevio::elev as e;

fn main() -> std::io::Result<()> {
    let elev_num_floors: u8 = 4;
    let elevator = e::Elevator::init("localhost:15657", elev_num_floors)?;
    println!("Elevator started:\n{:#?}", elevator);

    let poll_period = Duration::from_millis(25);

    let (call_button_tx, call_button_rx) = cbc::unbounded::<elevio::poll::CallButton>();
    {
        let elevator = elevator.clone();
        spawn(move || elevio::poll::call_buttons(elevator, call_button_tx, poll_period));
    }

    let (floor_sensor_tx, floor_sensor_rx) = cbc::unbounded::<u8>();
    {
        let elevator = elevator.clone();
        spawn(move || elevio::poll::floor_sensor(elevator, floor_sensor_tx, poll_period));
    }

    let (stop_button_tx, stop_button_rx) = cbc::unbounded::<bool>();
    {
        let elevator = elevator.clone();
        spawn(move || elevio::poll::stop_button(elevator, stop_button_tx, poll_period));
    }

    let (obstruction_tx, obstruction_rx) = cbc::unbounded::<bool>();
    {
        let elevator = elevator.clone();
        spawn(move || elevio::poll::obstruction(elevator, obstruction_tx, poll_period));
    }

    // ---- Controller state ----
    let mut current_floor: Option<u8> = elevator.floor_sensor();
    let mut target_floor: Option<u8> = None;
    let mut target_call: Option<u8> = None; // for å kunne slå av riktig lampe
    let mut obstructed: bool = false;
    let mut stop_active: bool = false;

    // Homing: hvis vi ikke er i en etasje, kjør ned til vi får floor sensor event.
    if current_floor.is_none() {
        elevator.motor_direction(e::DIRN_DOWN);
    } else {
        elevator.motor_direction(e::DIRN_STOP);
    }

    loop {
        cbc::select! {
            recv(call_button_rx) -> a => {
                let cb = a.unwrap();
                println!("Call: floor={}, call={}", cb.floor, cb.call);

                // Hvis stop er aktiv, ignorer nye ordre
                if stop_active { continue; }

                // Enkelt: ta imot ny target (overskriver evt gammel)
                target_floor = Some(cb.floor);
                target_call  = Some(cb.call);

                elevator.call_button_light(cb.floor, cb.call, true);

                // Hvis vi allerede står i samme etasje: server med en gang
                if let Some(cf) = current_floor {
                    if cf == cb.floor {
                        elevator.motor_direction(e::DIRN_STOP);
                        open_door_3s(&elevator, &mut obstructed);
                        elevator.call_button_light(cb.floor, cb.call, false);
                        target_floor = None;
                        target_call = None;
                    } else {
                        // ellers: start å kjøre mot target
                        let dir = if cb.floor > cf { e::DIRN_UP } else { e::DIRN_DOWN };
                        elevator.motor_direction(dir);
                    }
                }
            },

            recv(floor_sensor_rx) -> a => {
                let f = a.unwrap();
                current_floor = Some(f);
                elevator.floor_indicator(f);
                println!("Floor: {}", f);

                if stop_active {
                    elevator.motor_direction(e::DIRN_STOP);
                    continue;
                }

                if let Some(tf) = target_floor {
                    if f == tf {
                        // Arrived
                        elevator.motor_direction(e::DIRN_STOP);
                        open_door_3s(&elevator, &mut obstructed);

                        // Slå av riktig lampe (hvis vi vet hvilken knapp)
                        if let Some(call) = target_call {
                            elevator.call_button_light(tf, call, false);
                        }

                        target_floor = None;
                        target_call = None;
                    } else {
                        // Fortsett mot target
                        let dir = if tf > f { e::DIRN_UP } else { e::DIRN_DOWN };
                        // Hvis du vil, kan du stoppe motor ved obstruction, men vanligvis gjelder obstruction dør.
                        elevator.motor_direction(dir);
                    }
                } else {
                    // Ingen target: stopp
                    elevator.motor_direction(e::DIRN_STOP);
                }
            },

            recv(stop_button_rx) -> a => {
                let stop = a.unwrap();
                stop_active = stop;
                println!("Stop button: {}", stop);

                elevator.stop_button_light(stop);

                if stop {
                    // Stopp motor, nullstill ordre og lys
                    elevator.motor_direction(e::DIRN_STOP);
                    target_floor = None;
                    target_call = None;

                    for f in 0..elev_num_floors {
                        for c in 0..3 {
                            elevator.call_button_light(f, c, false);
                        }
                    }

                    // Hvis vi er i en etasje, åpne dør
                    if current_floor.is_some() {
                        elevator.door_light(true);
                    }
                } else {
                    // Slukk dør når stop slippes (enkel variant)
                    elevator.door_light(false);
                }
            },

            recv(obstruction_rx) -> a => {
                let ob = a.unwrap();
                obstructed = ob;
                println!("Obstruction: {}", ob);

                // En enkel og "riktig nok" håndtering: obstruction påvirker dør-tid (se open_door_3s)
                // Hvis du vil stoppe motor ved obstruction, kan du gjøre det her,
                // men i prosjektet brukes obstruction primært for dør.
            },
        }
    }
}

fn open_door_3s(elevator: &e::Elevator, obstructed: &mut bool) {
    elevator.door_light(true);

    // Hold døra åpen i 3s, men hvis obstruction er aktiv: forleng til den slippes
    let base = Duration::from_secs(3);
    let mut start = Instant::now();

    loop {
        // Hvis ikke obstructed: sjekk om tiden har gått
        if !*obstructed && start.elapsed() >= base {
            break;
        }

        // Hvis obstructed: reset timer når obstruction slipper
        if *obstructed {
            // vent litt, og la main-loopen oppdatere obstructed via event
            // (denne funksjonen har ikke tilgang til event-kanalen, så vi "poller" bare med sleep)
            std::thread::sleep(Duration::from_millis(20));
            // Når obstruction blir false (oppdatert av main via mutable ref?) så reset start:
            if !*obstructed {
                start = Instant::now();
            }
        } else {
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    elevator.door_light(false);
}
