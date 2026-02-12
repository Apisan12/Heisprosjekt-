use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use std::sync::Arc;
use crate::logic::logic_loop::LogicMsg;
use crate::domain::messages::NetState;

pub async fn receiver_task(
    socket: Arc<UdpSocket>,
    tx_logic: mpsc::Sender<LogicMsg>,
) {
    let mut buf = [0u8;1024];

    loop {
        let (len, _) = socket.recv_from(&mut buf).await.unwrap();

        let msg: NetState =
            bincode::deserialize(&buf[..len]).unwrap();

        println!("Fikk: {:?}",msg);
        tx_logic.send(LogicMsg::NetUpdate(msg)).await.ok();
    }
}
