#[derive(Debug)]
pub enum ElevatorState {
    Idle,
    Moving { target: u8 },
    DoorOpen,
    Stop,
}