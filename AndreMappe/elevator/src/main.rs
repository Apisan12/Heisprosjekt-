mod orders;
mod network;
mod driver;
mod config;
mod messages;
mod fsm;
mod tests;
mod init;

use driver_rust::elevio::elev::{self as e, DIRN_DOWN, DIRN_STOP};
use tokio::sync::{mpsc, watch};
use messages::{PeerState, ManagerMsg, FsmMsg, NewCall};
use orders::order_manager;
use orders::assigner;
use network::network::{create_socket, peer_state_receiver, peer_state_sender};
use driver::pollers::{spawn_input_pollers};
use driver::bridge::driver_bridge;
use fsm::fsm as f;

use crate::config::*;
// fn main() {
//     let world = test_world_realistic();
//     assigner::run_assigner(&world);

//     let world2 = test_world_stress();
//     assigner::run_assigner(&world2);
// }


#[tokio::main]
async fn main() -> std::io::Result<()> {
    // ID
    // Velger id med å kjøre "cargo run --id"
    // eksempel cargo run --1
    // TODO:
    // Lage en funskjon som lager ny ID basert på IP eller noe.
    // Må hvertfall kunne skille mellom de ulike heisene, men i tilegg kunne
    // initialisere med samme ID hvis det er en heis som har krasjet og startet på ny
    // slik at den har samme ID for å kunne recovere cab calls.
    let id: u8 = init::parse_id();
    // ID

    // Initialisere en heis
    // Kobler til en heis server som bruker ID for å ha forskjellige port
    // 
    // TODO:
    // Finne ut hvordan IP funker når det blir flere heiser
    // - Vil det være forskjellige IP 
    // - Hvis det er localhost, må annen måte enn IP brukes til å lage forskjellige ID
    // Flyttes inn i init?
    // Lage config for valg av heis etasjer?

    let elevator = init::init_elevator(id)?;
    // Initialisere en heis

    // Channels
    let (tx_manager, rx_manager, tx_fsm, rx_fsm, tx_peerstate, rx_peerstate) = init::init_channels(id, &elevator);

    // UDP socket
    // Lager UDP socket og tilater broadcast
    let socket = create_socket(NETWORK_PORT);
    socket.set_broadcast(true).unwrap();
    // UDP socket

    init::spawn_tasks(
        id,
        elevator.clone(),
        socket,
        tx_manager,
        rx_manager,
        tx_fsm,
        rx_fsm,
        tx_peerstate,
        rx_peerstate,
    );    

    // Loop for å holde main igang
    loop { tokio::time::sleep(std::time::Duration::from_secs(60)).await; }
    // Loop for å holde main igang

}


    // MANAGER KANAL
    // Lager manager kanal for å sende ManagerMsg.
    // Sender fra: 
    // - Heis input
    // - Andre noder i nettverket
    // - FSM
    // Mottar til:
    // - Order Manager
    // MANAGER KANAL

    // FSM kanal
    // TODO:
    // Lage FSM kanal som sender beskjeder til FSM
    // FSM kanal

    // Worldview kanal
    // Sender PeerState til alle noder, PeerState blir motat av order manager og lagt i worldview.
    // TODO:
    // Må ha en initial PeerState når kanalen opprettes, dette burde lages i init og ha en funskjon
    // som leser hvor heisen er osv når den blir startet.
    // Hvis heisen er mellom etasjer når den starter må den først gå til et floor
    // og deretter få en initial PeerState.
    // Worldview kanal

    // INPUT TRÅD
    // Lager tråd som tar imot input fra heis
    // Polles med det som var gitt i driver modulen, bruker en bridge til å gjøre det om til 
    // meldigner på tokio kanalene
    // TODO:
    // Legge til FSM kanalen når den er oprettet
    // INPUT TRÅD

    // Nettverk tråder
    // Lager tråd for å mota PeerState fra andre noder og sende til order_manager
    // Lager tråd for å sende PeerState til andre noder.
    // Nettverk tråder

    // Order Manager tråd
    // TODO:
    // Legge til for å sende på FSM kanal når den er lagd
    // Order Manager tråd

    // FSM tråd
    // TODO:
    // Lage FSM tråd
    // FSM tråd

// Returnerer etasjen heisen står i, hvis den er mellom etasjer kjøres den ned til den når en etasje
// og returnere denne etasjen.

