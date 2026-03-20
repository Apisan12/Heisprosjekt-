//! UDP-based networking for distributed elevator coordination.
//!
//! This module is responsible for:
//! - creating UDP broadcast sockets
//! - verifying basic local network functionality at startup
//! - recovering cab-call state from other nodes during startup
//! - broadcasting local elevator state
//! - receiving remote elevator state updates
//! - detecting disconnected and reconnected peers

use crate::config::{DISCONNECT_TIMEOUT, UDP_BROADCAST_PORT};
use crate::messages::{Call, ElevatorStatus, MsgToWorldManager, ElevatorId};
use socket2::{Domain, Protocol, Socket, Type};
use std::collections::{HashMap, HashSet};
use std::net::{SocketAddr, UdpSocket as StdUdpSocket};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, watch};
use tokio::time::Instant;
use tokio::time::timeout;


/// Recover known cab calls for the local node during startup.
///
/// This function listens for a short time window on the shared network
/// port and collects any cab calls from broadcast elevator states that
/// belong to `node_id`.
/// Returns the recovered set of calls.
pub async fn recover_startup_state(node_id: ElevatorId) -> HashSet<Call> {
    // Create UDP socket (same way the network manager does)
    let socket = create_socket(UDP_BROADCAST_PORT);
    // let socket = Arc::new(socket);

    let mut recovered = HashSet::new();
    let mut buf = [0u8; 4096];

    // Listen window for recovery
    let deadline = Instant::now() + DISCONNECT_TIMEOUT;

    while Instant::now() < deadline {
        match timeout(Duration::from_millis(100), socket.recv_from(&mut buf)).await {
            Ok(Ok((len, _addr))) => {
                if let Ok(status) = bincode::deserialize::<ElevatorStatus>(&buf[..len]) {
                    for call in &status.known_cab_calls {
                        if call.call_id.elevator_id == node_id {
                            recovered.insert(call.clone());
                        }
                    }
                }
            }

            // recv_from error
            Ok(Err(_)) => {}

            // timeout expired
            Err(_) => {}
        }
    }

    recovered
}

/// Create a UDP socket configured for local broadcast-based communication.
/// The returned socket is wrapped in `Arc` so it can be shared across async tasks if needed.
pub fn create_socket(port: u16) -> Arc<UdpSocket> {
    let addr: SocketAddr = format!("0.0.0.0:{port}").parse().expect("invalid addr");

    let socket =
        Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)).expect("socket create failed");

    socket.set_reuse_address(true).expect("reuse addr failed");

    socket.bind(&addr.into()).expect("bind failed");

    let std_socket: StdUdpSocket = socket.into();
    std_socket.set_nonblocking(true).unwrap();

    let tokio_socket = UdpSocket::from_std(std_socket).expect("tokio socket failed");

    tokio_socket.set_broadcast(true).expect("broadcast failed");

    Arc::new(tokio_socket)
}


/// Run the network manager for a single elevator node.
///
/// Responsibilities:
/// - observe changes to local elevator state
/// - periodically broadcast the local state to peers
/// - receive and deserialize remote elevator states
/// - track last-seen timestamps for known peers
/// - notify the world manager when peers disconnect or reconnect
///
/// The manager runs indefinitely until the task is cancelled.
pub async fn network_manager(
    mut rx_network: watch::Receiver<ElevatorStatus>,
    tx_world_view_msg: mpsc::Sender<MsgToWorldManager>,
) {
    let mut tick = tokio::time::interval(Duration::from_millis(500));
    let mut buf = [0u8; 4096];

    let mut local_elevator_state = rx_network.borrow().clone();
    let mut known_elevators: HashMap<ElevatorId, (Instant, ElevatorStatus)> = HashMap::new();
    let mut disconnected_elevators: HashMap<ElevatorId, ElevatorStatus> = HashMap::new();

    let socket = create_socket(UDP_BROADCAST_PORT);

    loop {
        tokio::select! {

        Ok(_) = rx_network.changed() => {
        let new_state = rx_network.borrow().clone();

        if new_state != local_elevator_state {
            print_state_change(&local_elevator_state, &new_state);
            local_elevator_state = new_state;
        }
        }

_ = tick.tick() => {
    let now = Instant::now();
    let mut disconnected = Vec::new();

    known_elevators.retain(|elev_id, (last_seen, status)| {
        if now.duration_since(*last_seen) >= DISCONNECT_TIMEOUT {
            println!("Elevator disconnected: {:?}", elev_id);

            disconnected_elevators.insert(*elev_id,status.clone());
            disconnected.push(*elev_id);
            false
        } else {
            true
        }
    });

    for elev_id in disconnected {
        let _ = tx_world_view_msg
            .send(MsgToWorldManager::AddDisconnectedElevator(elev_id))
            .await;
    }


    let bytes = bincode::serialize(&local_elevator_state).unwrap();
    let _ = socket
                .send_to(&bytes, format!("255.255.255.255:{UDP_BROADCAST_PORT}"))
                .await;
}

Ok((len, _)) = socket.recv_from(&mut buf) => {
    if let Ok(remote_elevator_state) =
        bincode::deserialize::<ElevatorStatus>(&buf[..len])
    {
        if remote_elevator_state.elevator_id == local_elevator_state.elevator_id {
            continue;
        }

        let elev_id = remote_elevator_state.elevator_id;

        if disconnected_elevators.remove(&elev_id).is_some() {
            println!("Known elevator reconnected: {:?}", elev_id);
            let _ = tx_world_view_msg
                .send(MsgToWorldManager::RemoveDisconnectedElevator(elev_id))
                .await;
        } else if !known_elevators.contains_key(&elev_id) {
            println!("New elevator on network: {:?}", elev_id);
        }



        known_elevators.insert(elev_id, (Instant::now(), remote_elevator_state.clone()));

        let _ = tx_world_view_msg
                    .send(MsgToWorldManager::NewRemoteElevatorStatus(remote_elevator_state))
                    .await;
    }
    if let Ok(initializing_elevator) =
        bincode::deserialize::<ElevatorId>(&buf[..len])
    {
        if let Some(elevator) = disconnected_elevators.get(&initializing_elevator) {
            let recovered_cab_calls = elevator.cab_calls.clone();
            let bytes = bincode::serialize(&recovered_cab_calls).unwrap();
            let _ = socket
                    .send_to(&bytes, format!("255.255.255.255:{UDP_BROADCAST_PORT}"));

        }

    }

}
}
}
}


/// Print selected local elevator state changes for debugging.
fn print_state_change(old: &ElevatorStatus, new: &ElevatorStatus) {
    if old.behaviour != new.behaviour {
        println!("Behaviour: {:?} -> {:?}", old.behaviour, new.behaviour);
    }

    if old.floor != new.floor {
        println!("Floor: {} -> {}", old.floor, new.floor);
    }

    if old.direction != new.direction {
        println!("Direction: {:?} -> {:?}", old.direction, new.direction);
    }

    if old.has_faults != new.has_faults {
        println!("Obstruction changed: {}", new.has_faults);
    }
}
