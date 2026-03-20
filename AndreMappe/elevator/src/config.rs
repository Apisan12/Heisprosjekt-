// This module holds all static program parameters used in one place.
use std::time::Duration;

// Number of floors in the elevator.
pub const ELEVATOR_NUM_FLOORS: u8 = 4;
pub const BOTTOM_FLOOR: u8 = 0;
pub const TOP_FLOOR: u8 = ELEVATOR_NUM_FLOORS - 1;

// Duration between elevator hardware polls
pub const ELEVTOR_POLL_TIME: Duration = Duration::from_millis(25);

// Ports
pub const BASE_DRIVER_PORT: u32 = 15657;
pub const UDP_BROADCAST_PORT: u16 = 30000;

// Timers
pub const DOOR_TIMEOUT: Duration = Duration::from_secs(3);
pub const TRAVEL_TIMEOUT: Duration = Duration::from_secs(6);
pub const DISCONNECT_TIMEOUT: Duration = Duration::from_secs(5);

