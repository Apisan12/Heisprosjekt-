mod domain;
mod logic;
mod network;

use std::sync::Arc;

use tokio::net::UdpSocket;
use tokio::sync::{mpsc, watch};
use domain::messages::{NetState, Call};
use logic::logic_loop::{logic_loop, LogicMsg};
use tokio::io::{self,AsyncBufReadExt};
use network::socket::create_socket;

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
    }
}

#[tokio::main]
async fn main() {

    let (tx_logic, rx_logic) = mpsc::channel::<LogicMsg>(32);
    let (tx_snapshot, rx_snapshot) =
        watch::channel(NetState { id:0, calls: Default::default() });

    let socket = create_socket(30000);
    socket.set_broadcast(true).unwrap();


    let id: u8 = std::env::args()
    .nth(1)
    .expect("missing id")
    .parse()
    .expect("id must be number");

    tokio::spawn(network::receiver::receiver_task(socket.clone(), tx_logic.clone()));
    tokio::spawn(network::sender::sender_task(socket.clone(), rx_snapshot));
    tokio::spawn(logic_loop(id, rx_logic, tx_snapshot));
    tokio::spawn(stdin_input_loop(tx_logic.clone()));

    loop { tokio::time::sleep(std::time::Duration::from_secs(60)).await; }
}
