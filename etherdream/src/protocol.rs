//! Network models associated with the Etherdream communication protocol.
//!
//! The Etherdream protocol definition can be found at:
//! https://ether-dream.com/protocol.html
use std::fmt;

// Point data type aliases
pub type X = i16;
pub type Y = i16;
pub type R = u16;
pub type G = u16;
pub type B = u16;

// Network Ports
pub const BROADCAST_PORT: u16 = 7654;
pub const CLIENT_PORT: u16    = 7765;

// Command signals
pub const COMMAND_BEGIN: u8        = b'b';
pub const COMMAND_CLEAR: u8        = b'c';
pub const COMMAND_DATA: u8         = b'd';
pub const COMMAND_ESTOP: u8        = b'0';
pub const COMMAND_PING: u8         = b'?';
pub const COMMAND_PREPARE: u8      = b'p';
pub const COMMAND_QUEUE_RATE: u8   = b'q';
pub const COMMAND_STOP: u8         = b's';

#[repr(u8)]
#[derive( Clone, Copy, Debug, PartialEq )]
pub enum Command {
  Begin     = COMMAND_BEGIN,
  Clear     = COMMAND_CLEAR,
  Data      = COMMAND_DATA,
  Estop     = COMMAND_ESTOP,
  Ping      = COMMAND_PING,
  Prepare   = COMMAND_PREPARE,
  QueueRate = COMMAND_QUEUE_RATE,
  Stop      = COMMAND_STOP
}

impl TryFrom<u8> for Command {
  type Error = u8;

  fn try_from( byte: u8 ) -> Result<Self, Self::Error> {
    match byte {
      COMMAND_BEGIN => Ok( Command::Begin ),
      COMMAND_CLEAR => Ok( Command::Clear ),
      COMMAND_DATA => Ok( Command::Data ),
      COMMAND_ESTOP => Ok( Command::Estop ),
      COMMAND_PING => Ok( Command::Ping ),
      COMMAND_PREPARE => Ok( Command::Prepare ),
      COMMAND_QUEUE_RATE => Ok( Command::QueueRate ),
      COMMAND_STOP => Ok( Command::Stop ),
      byte => Err( byte )
    }
  }
}

// Control signals
pub const CONTROL_ACK: u8          = b'a';
pub const CONTROL_NAK_ESTOP: u8    = b'!';
pub const CONTROL_NAK_FULL: u8     = b'F';
pub const CONTROL_NAK_INVALID: u8  = b'I';

#[repr(u8)]
#[derive( Clone, Copy, Debug, PartialEq )]
pub enum ControlSignal {
  Ack         = CONTROL_ACK,
  NakEstop    = CONTROL_NAK_ESTOP,
  NakFull     = CONTROL_NAK_FULL,
  NakInvalid  = CONTROL_NAK_INVALID
}

impl TryFrom<u8> for ControlSignal {
  type Error = u8;

  fn try_from( byte: u8 ) -> Result<Self, Self::Error> {
    match byte {
      CONTROL_ACK => Ok( ControlSignal::Ack ),
      CONTROL_NAK_ESTOP => Ok( ControlSignal::NakEstop ),
      CONTROL_NAK_FULL => Ok( ControlSignal::NakFull ),
      CONTROL_NAK_INVALID => Ok( ControlSignal::NakInvalid ),
      byte => Err( byte )
    }
  }
}

// Network message sizes
pub const INTRINSIC_BYTES_SIZE: usize   = 16;
pub const POINT_DATA_BYTES_SIZE: usize  = 18;
pub const RESPONSE_BYTES_SIZE: usize    = 22;
pub const STATE_BYTES_SIZE: usize       = 20;

pub const BROADCAST_BYTES_SIZE: usize = INTRINSIC_BYTES_SIZE + STATE_BYTES_SIZE;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Intrinsics

/// Describes an Etherdream DAC's intrinsic properties, as provided by its
/// network broadcast message.
#[derive( Clone, Copy, Debug, Default )]
pub struct Intrinsics {
  /// MAC address of the Etherdream DAC.
  pub mac_address: MacAddress,
  /// Hardware and software version for the Etherdream DAC.
  pub version: Version,
  /// Maximum capacity of the Etherdream DACs point buffer.
  pub buffer_capacity: u16,
  /// Maximum number of points the Etherdream DAC can process per second.
  pub max_points_per_second: u32
}

impl Intrinsics {
  /// Decodes a slice of bytes into an `Intrinsics` model.
  pub fn from_bytes( bytes: &[u8] ) -> Self {
    Self{
      buffer_capacity: u16::from_le_bytes([ bytes[10], bytes[11] ]),
      mac_address: MacAddress::new([ bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5] ]),
      max_points_per_second: u32::from_le_bytes([ bytes[12], bytes[13], bytes[14], bytes[15] ]),
      version: Version::new(
        u16::from_le_bytes([ bytes[6], bytes[7] ]),
        u16::from_le_bytes([ bytes[8], bytes[9] ])
      )
    }
  }
}

#[derive( Clone, Copy, Debug, Default )]
pub struct State {
  // [Unknown]
  pub protocol: u8,
  // The current light engine state of the DAC
  pub light_engine_state: LightEngineState,
  // The current playback state of the DAC
  pub playback_state: PlaybackState,
  // The current playback source of the DAC
  pub source: Source,
  // The current light engine flags. DAC is ready if all flags are zero.
  // Bits:
  //  [0]: Emergency stop occurred due to E-Stop packet or invalid command.
  //  [1]: Emergency stop occurred due to E-Stop input to projector.
  //  [2]: Emergency stop input to projector is currently active.
  //  [3]: Emergency stop occurred due to overtemperature condition.
  //  [4]: Overtemperature condition is currently active.
  //  [5]: Emergency stop occurred due to loss of Ethernet link.
  pub light_engine_flags: u16,
  // The current playback flags. Bits will be non-zero during normal operation.
  // Bits:
  //  [0]: Shutter state: 0 = closed, 1 = open.
  //  [1]: Underflow. 1 if the last stream ended with underflow, rather than a
  //			 Stop command. Reset to zero by the Prepare command.
  //  [2]: E-Stop. 1 if the last stream ended because the E-Stop state was
  //       entered. Reset to zero by the Prepare command.
  pub playback_flags: u16,
  // [Unknown]
  pub source_flags: u16,
  // The current number of points that are buffered in the DAC.
  pub points_buffered: u16,
  // The current number of points that the DAC is processing per second.
  pub points_per_second: u32,
  // The total number of points that the DAC has processed.
  pub points_lifetime: u32
}

impl State {
  /// Decodes a slice of bytes into a `State` model.
  pub fn from_bytes( bytes: &[u8] ) -> Self {
    Self{
      protocol: bytes[0],
      light_engine_state: bytes[1].into(),
      playback_state: bytes[2].into(),
      source: bytes[3].into(),
      light_engine_flags: u16::from_le_bytes([ bytes[4], bytes[5] ]),
      playback_flags: u16::from_le_bytes([ bytes[6], bytes[7] ]),
      source_flags: u16::from_le_bytes([ bytes[8], bytes[9] ]),
      points_buffered: u16::from_le_bytes([ bytes[10], bytes[11] ]),
      points_per_second: u32::from_le_bytes([ bytes[12], bytes[13], bytes[14], bytes[15] ]),
      points_lifetime: u32::from_le_bytes([ bytes[16], bytes[17], bytes[18], bytes[19] ])
    }
  }

  pub fn is_ready( &self ) -> bool {
    self.light_engine_flags == 0 &&
      self.light_engine_state == LightEngineState::Ready &&
      self.playback_state != PlaybackState::Idle
  }

  pub fn is_playing( &self ) -> bool {
    self.is_ready() && self.playback_state == PlaybackState::Playing
  }

  pub fn is_e_stop( &self ) -> bool { ( 1 & ( self.playback_flags >> 2 ) ) == 1 }
  pub fn is_shutter_open( &self ) -> bool { ( 1 & ( self.playback_flags >> 0 ) ) == 1 }
  pub fn is_underflow( &self ) -> bool { ( 1 & ( self.playback_flags >> 1 ) ) == 1 }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Light Engine State

#[derive( Clone, Copy, Debug, PartialEq )]
pub enum LightEngineState {
  Ready,
  WarmUp,
  CoolDown,
  Estop
}

impl LightEngineState {
  pub fn from_byte( byte: u8 ) -> Self {
    match byte {
      0 => LightEngineState::Ready,
      1 => LightEngineState::WarmUp,
      2 => LightEngineState::CoolDown,
      3 | _ => LightEngineState::Estop
    }
  }
}

impl From<u8> for LightEngineState {
  fn from( byte: u8 ) -> LightEngineState { LightEngineState::from_byte( byte ) }
}

impl Default for LightEngineState {
  fn default() -> Self { Self::Ready }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Playback State

#[derive( Clone, Copy, Debug, PartialEq )]
pub enum PlaybackState {
  Idle,
  Prepared,
  Playing
}

impl PlaybackState {
  pub fn from_byte( byte: u8 ) -> Self {
    match byte {
      0 => PlaybackState::Idle,
      1 => PlaybackState::Prepared,
      2 | _ => PlaybackState::Playing
    }
  }
}

impl From<u8> for PlaybackState {
  fn from( byte: u8 ) -> PlaybackState { PlaybackState::from_byte( byte ) }
}

impl Default for PlaybackState {
  fn default() -> Self { Self::Idle }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Source

#[derive( Clone, Copy, Debug, PartialEq )]
pub enum Source {
  Network,
  Ilda,
  Internal
}

impl Source {
  pub fn from_byte( byte: u8 ) -> Self {
    match byte {
      0 => Source::Network,
      1 => Source::Ilda,
      2 | _ => Source::Internal
    }
  }
}

impl From<u8> for Source {
  fn from( byte: u8 ) -> Source { Source::from_byte( byte ) }
}

impl Default for Source {
  fn default() -> Self { Self::Network }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Version

#[derive( Clone, Copy, Debug, PartialEq )]
pub struct Version {
  pub hardware: u16,
  pub software: u16
}

impl Version {
  pub fn new( hardware_version: u16, software_version: u16 ) -> Self {
    Self{ hardware: hardware_version, software: software_version }
  }
}

impl Default for Version {
  fn default() -> Self {
    Self{ hardware: 0, software: 0 }
  }
}

impl From<[u16;2]> for Version {
  fn from( bytes: [u16;2] ) -> Version {
    Version::new( bytes[0], bytes[1] )
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  MAC Address

#[derive( Clone, Copy, Debug, PartialEq )]
pub struct MacAddress {
  inner: [u8; 6]
}

impl MacAddress {
  pub fn new( address: [u8; 6] ) -> Self { Self{ inner: address } }
  pub fn as_slice( &self ) -> &[u8] { &self.inner }
}

impl Default for MacAddress {
  fn default() -> Self { Self{ inner: [ 0, 0, 0, 0, 0, 0 ] } }
}

impl fmt::Display for MacAddress {
  fn fmt( &self, f: &mut fmt::Formatter ) -> fmt::Result {
    write!( f, "{:02X?}", self.inner )
  }
}

impl From<[u8;6]> for MacAddress {
  fn from( bytes: [u8;6] ) -> Self { Self::new( bytes ) }
}