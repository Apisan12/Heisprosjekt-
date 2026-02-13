mod domain;
mod logic;
mod network;
mod driver;

use std::sync::Arc;
use std::thread::spawn;
use std::time::Duration;

use driver_rust::elevio::elev as e;
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, watch};
use domain::messages::{NetState, Call};
use logic::logic_loop::{logic_loop, LogicMsg};
use tokio::io::{self,AsyncBufReadExt};
use network::socket::create_socket;
use driver::pollers::{spawn_pollers, PollReceivers};

use crate::driver::bridge::driver_bridge;
use crate::driver::pollers;

pub async fn stdin_input_loop(
    tx_logic: tokio::sync::mpsc::Sender<LogicMsg>,
) {
    let stdin = io::BufReader::new(io::stdin());
    let mut lines = stdin.lines();

    println!("Skriv: <floor> <call>");

    while let Ok(Some(line)) = lines.next_line().await {

        let parts: Vec<_> = line.split_whitespace().collect();

        if parts.len() != 2 {
            println!("Format: floor call");
            continue;
        }

        let floor: u8 = match parts[0].parse() {
            Ok(v) => v,
            Err(_) => {
                println!("Ugyldig floor");
                continue;
            }
        };

        let call: u8 = match parts[1].parse() {
            Ok(v) => v,
            Err(_) => {
                println!("Ugyldig call");
                continue;
            }
        };

        let call = Call { floor, call };

        tx_logic.send(LogicMsg::LocalButton(call)).await.ok();
        println!("Call sendt.");
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let id: u8 = std::env::args()
    .nth(1)
    .expect("missing id")
    .parse()
    .expect("id must be number");

    let port: u32 = 15656 + id as u32;
    let addr = format!("localhost:{}", port);
    let elev_num_floors = 4;
    let elevator = e::Elevator::init(&addr, elev_num_floors)?;
    println!("Elevator started:\n{:#?}", elevator);


    let poll_period = Duration::from_millis(25);
    let pollers = spawn_pollers(elevator.clone(), poll_period);

    let (tx_logic, rx_logic) = mpsc::channel::<LogicMsg>(32);
    let (tx_snapshot, rx_snapshot) =
        watch::channel(NetState { id, calls: Default::default() });

    let socket = create_socket(30000);
    socket.set_broadcast(true).unwrap();

    tokio::spawn(network::receiver::receiver_task(socket.clone(), tx_logic.clone()));
    tokio::spawn(network::sender::sender_task(socket.clone(), rx_snapshot));
    tokio::spawn(logic_loop(id, rx_logic, tx_snapshot, elevator.clone()));
    tokio::spawn(driver_bridge(pollers, tx_logic.clone()));

    loop { tokio::time::sleep(std::time::Duration::from_secs(60)).await; }
}