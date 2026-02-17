use socket2::{Socket, Domain, Type, Protocol};
use std::net::{SocketAddr, UdpSocket as StdUdpSocket};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, watch};
use crate::messages::{ManagerMsg, PeerState};

// Lager UDP socket
// Greier for å kunne åpne flere sockets på samme IP på windows (For å kjøre flere heisprogram på samme IP)
pub fn create_socket(port: u16) -> Arc<UdpSocket> {

    let addr: SocketAddr = format!("0.0.0.0:{port}")
        .parse()
        .expect("invalid addr");

    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))
        .expect("socket create failed");

    // Viktig på Windows når flere instanser kjører
    socket.set_reuse_address(true).expect("reuse addr failed");

    socket.bind(&addr.into()).expect("bind failed");

    let std_socket: StdUdpSocket = socket.into();
    std_socket.set_nonblocking(true).unwrap();

    let tokio_socket = UdpSocket::from_std(std_socket)
        .expect("tokio socket failed");

    tokio_socket.set_broadcast(true)
        .expect("broadcast failed");

    Arc::new(tokio_socket)
}

// Tråd for ta imot PeerState og sende til manager på manager kanalen med NetUpdate beskjed
pub async fn peer_state_receiver(
    socket: Arc<UdpSocket>,
    tx_manager: mpsc::Sender<ManagerMsg>,
) {
    let mut buf = [0u8;1024];

    loop {
        let (len, _) = socket.recv_from(&mut buf).await.unwrap();

        let msg: PeerState =
            bincode::deserialize(&buf[..len]).unwrap();

        // println!("Fikk: {:?}",msg);
        tx_manager.send(ManagerMsg::NetUpdate(msg)).await.ok();
    }
}

//Tråd for å sende PeerState til de andre nodene
pub async fn peer_state_sender(
    socket: Arc<UdpSocket>,
    rx_peerstate: watch::Receiver<PeerState>,
) {
    let mut tick = tokio::time::interval(Duration::from_millis(100));

    loop {
        tick.tick().await;

        let state = rx_peerstate.borrow().clone();
        // println!("Sente: {:?}",state);
        let bytes = bincode::serialize(&state).unwrap();

        socket.send_to(&bytes, "255.255.255.255:30000").await.unwrap();
    }
}
