// src/messages.rs

#[derive(Debug, Clone)]
pub enum Command {
    GoToFloor(u8),
    OpenDoor,
    Stop,
}

#[derive(Debug)]
pub enum FsmEvent {
    AtFloor(u8),
    DoorTimeout,
    Obstruction(bool),
    Idle,
}
