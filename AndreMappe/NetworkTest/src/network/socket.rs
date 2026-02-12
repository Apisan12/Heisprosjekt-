use socket2::{Socket, Domain, Type, Protocol};
use std::net::{SocketAddr, UdpSocket as StdUdpSocket};
use tokio::net::UdpSocket;
use std::sync::Arc;

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
