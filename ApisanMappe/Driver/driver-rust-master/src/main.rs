use std::thread::*;
use std::time::*;

use crossbeam_channel as cbc;

use driver_rust::elevio;
use driver_rust::elevio::elev as e;

fn main() -> std::io::Result<()> {
    let num_floors: u8 = 4;
    let elevator = Elevator::init("localhost:15657", num_floors)?;
    println!("Connected to {}", elevator);

    // 1) "Homing": hvis vi ikke er i en etasje, kjør ned til vi treffer en.
    if elevator.floor_sensor().is_none() {
        elevator.motor_direction(DIRN_DOWN);
        loop {
            if let Some(f) = elevator.floor_sensor() {
                elevator.motor_direction(DIRN_STOP);
                elevator.floor_indicator(f);
                break;
            }
            sleep(Duration::from_millis(20));
        }
    }

    let poll_period = Duration::from_millis(50);

    loop {
        // 2) Vent på en ny bestilling (én av gangen)
        let (target_floor, target_call) = wait_for_any_button(&elevator, num_floors, poll_period);

        // Skru på lys for knappen vi tok imot
        elevator.call_button_light(target_floor, target_call, true);

        // 3) Kjør til etasjen
        go_to_floor(&elevator, target_floor, poll_period);

        // 4) "Serve": stopp + dør 3 sek
        serve_floor(&elevator, target_floor);

        // 5) Slukk lys og clear
        elevator.call_button_light(target_floor, target_call, false);
    }
}

/// Poller alle knapper og returnerer første (floor, call) som er trykket.
fn wait_for_any_button(e: &Elevator, num_floors: u8, poll_period: Duration) -> (u8, u8) {
    loop {
        for f in 0..num_floors {
            for c in [HALL_UP, HALL_DOWN, CAB] {
                // Filtrer bort "ugyldige" hall-knapper i endene (ofte ikke fysisk tilgjengelig)
                if (f == 0 && c == HALL_DOWN) || (f == num_floors - 1 && c == HALL_UP) {
                    continue;
                }
                if e.call_button(f, c) {
                    println!("Order received: floor={}, call={}", f, c);
                    return (f, c);
                }
            }
        }
        sleep(poll_period);
    }
}

/// Kjører motor opp/ned til vi når target_floor.
fn go_to_floor(e: &Elevator, target_floor: u8, poll_period: Duration) {
    loop {
        // Oppdater nåværende etasje (kan være None mellom etasjer)
        if let Some(cur) = e.floor_sensor() {
            e.floor_indicator(cur);

            if cur == target_floor {
                e.motor_direction(DIRN_STOP);
                println!("Arrived at floor {}", cur);
                return;
            }

            // Sett retning mot target
            let dir = if target_floor > cur { DIRN_UP } else { DIRN_DOWN };
            e.motor_direction(dir);
        } else {
            // Mellom etasjer: bare vent litt (motorretning er allerede satt)
        }

        sleep(poll_period);
    }
}

/// Stopper, åpner dør og holder åpen i ~3 sek.
fn serve_floor(e: &Elevator, floor: u8) {
    e.motor_direction(DIRN_STOP);
    e.floor_indicator(floor);

    e.door_light(true);
    let open_time = Duration::from_secs(3);
    let start = Instant::now();
    while start.elapsed() < open_time {
        // Hvis du vil: her kan du sjekke obstruction og forlenge tiden
        sleep(Duration::from_millis(20));
    }
    e.door_light(false);
}