// This module holds all static program parameters used in one place.
use std::time::Duration;

// Number of floors in the elevator.
pub const ELEV_NUM_FLOORS: u8 = 4;

// Duration between elevator hardware polls
pub const ELEV_POLL: Duration = Duration::from_millis(25);

// Network
pub const BASE_ELEVATOR_PORT: u32 = 15656;
pub const NETWORK_PORT: u16 = 30000;


