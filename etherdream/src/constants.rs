//! Constants that map to the Etherdream network protocol. The protocol
//! definitions can be found at: https://ether-dream.com/protocol.html

// Etherdream command signals
pub const ETHERDREAM_COMMAND_BEGIN: u8        = b'b';
pub const ETHERDREAM_COMMAND_CLEAR: u8        = b'c';
pub const ETHERDREAM_COMMAND_DATA: u8         = b'd';
pub const ETHERDREAM_COMMAND_ESTOP: u8        = b'0';
pub const ETHERDREAM_COMMAND_PING: u8         = b'?';
pub const ETHERDREAM_COMMAND_PREPARE: u8      = b'p';
pub const ETHERDREAM_COMMAND_QUEUE_RATE: u8   = b'q';
pub const ETHERDREAM_COMMAND_STOP: u8         = b's';

// Etherdream control signals
pub const ETHERDREAM_CONTROL_ACK: u8          = b'a';
pub const ETHERDREAM_CONTROL_NAK_ESTOP: u8    = b'!';
pub const ETHERDREAM_CONTROL_NAK_FULL: u8     = b'F';
pub const ETHERDREAM_CONTROL_NAK_INVALID: u8  = b'I';

// Etherdream network payload sizes
pub const ETHERDREAM_INTRINSIC_BYTES: usize = 16;
pub const ETHERDREAM_RESPONSE_BYTES: usize  = 22;
pub const ETHERDREAM_STATE_BYTES: usize     = 20;

// Etherdream broadcast
pub const ETHERDREAM_BROADCAST_PORT: u16    = 7654;
pub const ETHERDREAM_BROADCAST_BYTES: usize = ETHERDREAM_INTRINSIC_BYTES + ETHERDREAM_STATE_BYTES;

// Etherdream client
pub const ETHERDREAM_CLIENT_PORT: u16 = 7765;