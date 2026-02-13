use tokio::net::UdpSocket;
use tokio::sync::watch;
use std::time::Duration;
use std::sync::Arc;
use crate::domain::messages::NetState;

pub async fn sender_task(
    socket: Arc<UdpSocket>,
    rx_snapshot: watch::Receiver<NetState>,
) {
    let mut tick = tokio::time::interval(Duration::from_millis(100));

    loop {
        tick.tick().await;

        let state = rx_snapshot.borrow().clone();
        // println!("Sente: {:?}",state);
        let bytes = bincode::serialize(&state).unwrap();

        socket.send_to(&bytes, "255.255.255.255:30000").await.unwrap();
    }
}
