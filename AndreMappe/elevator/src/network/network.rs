use socket2::{Socket, Domain, Type, Protocol};
use std::net::{SocketAddr, UdpSocket as StdUdpSocket};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, watch};
use crate::config::NETWORK_PORT;
use crate::messages::{ElevStatus, MsgToWorldView};

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

pub async fn network_manager(
    mut rx_network: watch::Receiver<ElevStatus>,
    tx_world_view_msg: mpsc::Sender<MsgToWorldView>,
) {
    let mut tick = tokio::time::interval(Duration::from_millis(100));
    let mut buf = [0u8; 4096];

    let mut local_elev_state = rx_network.borrow().clone();

    let socket = create_socket(NETWORK_PORT);

    loop {
        tokio::select! {

            Ok(_) = rx_network.changed() => {
                local_elev_state = rx_network.borrow().clone();
            }

            _ = tick.tick() => {
                let bytes = bincode::serialize(&local_elev_state).unwrap();
                let _ = socket
                            .send_to(&bytes, "255.255.255.255:30000")
                            .await;
            }

            Ok((len, _)) = socket.recv_from(&mut buf) => {
                if let Ok(remote_elev_state) =
                    bincode::deserialize::<ElevStatus>(&buf[..len])
                {
                    let _ = tx_world_view_msg
                                .send(MsgToWorldView::NewRemoteElevState(remote_elev_state))
                                .await;
                }
            }
        }
    }
}
