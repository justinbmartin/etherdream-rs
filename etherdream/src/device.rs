use std::fmt;

pub const DEVICE_BYTES_SIZE: usize = 36;
pub const DEVICE_STATE_BYTES_SIZE: usize = 20;
pub const DEFAULT_PORT: u16 = 7765;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Light Engine State

#[repr( u8 )]
#[derive( Clone, Copy, Debug, PartialEq )]
pub enum LightEngineState {
  Ready = 0,
  WarmUp = 1,
  CoolDown = 2,
  Estop = 3
}

impl Default for LightEngineState {
  fn default() -> Self {
    Self::Ready
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Playback State

#[repr( u8 )]
#[derive( Clone, Copy, Debug, PartialEq )]
pub enum PlaybackState {
  Idle = 0,
  Prepared = 1,
  Playing = 2
}

impl Default for PlaybackState {
  fn default() -> Self {
    Self::Idle
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Source

#[repr( u8 )]
#[derive( Clone, Copy, Debug, PartialEq )]
pub enum Source {
  Network = 0,
  Ilda = 1,
  Internal = 2
}

impl Default for Source {
  fn default() -> Self {
    Self::Network
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Version

#[repr( C )]
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

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  MAC Address

#[repr( C )]
#[derive( Clone, Copy, Debug, PartialEq )]
pub struct MacAddress {
  address: [u8; 6]
}

impl MacAddress {
  pub fn new( address: [u8; 6] ) -> Self {
    Self{ address }
  }
}

impl From<MacAddress> for [u8; 6] {
  fn from( mac_address: MacAddress ) -> Self {
    mac_address.address
  }
}

impl fmt::Display for MacAddress {
  fn fmt( &self, f: &mut fmt::Formatter ) -> fmt::Result {
    write!( f, "{:02X?}", self.address )
  }
}

// ====================================================================== State

/// Encodes the Etherdream DAC state, received as a part of all DAC responses.
/// Reference: https://ether-dream.com/protocol.html
#[repr( C )]
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
  pub playback_flags: u8,

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
  pub fn from_bytes( bytes: [u8; DEVICE_STATE_BYTES_SIZE] ) -> Self {
    // SAFETY:
    // * `State` has the same size as `[u8; DEVICE_STATE_BYTES_SIZE]`.
    // * `[u8; DEVICE_STATE_BYTES_SIZE]` has no alignment requirement.
    unsafe{ std::mem::transmute( bytes ) }
  }
}

// ===================================================================== Device

/// Encodes a device response from an Etherdream DAC discovery broadcast. 
/// Reference: https://ether-dream.com/protocol.html
#[repr( C )]
#[derive( Clone, Copy, Debug )]
pub struct Device {
  /// MAC address of the DAC
  pub mac_address: MacAddress,

  /// Hardware and software version of the DAC
  pub version: Version,

  /// Maximum capacity of the DAC point data buffer
  pub buffer_capacity: u16,

  /// Maximum number of points the DAC can process per second
  pub max_points_per_second: u32,

  /// The current device state
  pub state: State
}

impl Device {
  pub fn from_bytes( bytes: [u8; DEVICE_BYTES_SIZE] ) -> Device {
    // SAFETY:
    // * `Device` has the same size as `[u8; DEVICE_BYTES_SIZE]`.
    // * `[u8; DEVICE_BYTES_SIZE]` has no alignment requirement.
    unsafe{ std::mem::transmute( bytes ) }
  }

  pub fn to_bytes( self ) -> [u8; DEVICE_BYTES_SIZE] {
    // SAFETY:
    // * `Device` has the same size as `[u8; DEVICE_BYTES_SIZE]`.
    // * `[u8; DEVICE_BYTES_SIZE]` has no alignment requirement.
    unsafe{ std::mem::transmute( self ) }
  }
}