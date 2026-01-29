//! Network data models associated with Etherdream devices.
//!
//! The Etherdream protocol definition can be found at:
//! https://ether-dream.com/protocol.html
use std::fmt;
use std::net::SocketAddr;

use crate::constants::*;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Device

/// Container that encodes information about an Etherdream device and its
/// run-time state.
#[derive( Clone, Copy, Debug )]
pub struct Device {
  /// The remote socket address that the Etherdream device is listening on.
  address: SocketAddr,

  /// The intrinsic Etherdream device properties.
  intrinsics: Intrinsics,

  /// The Etherdream device engine state.
  state: State
}

impl Device {
  /// Creates a new `Device` from a socket address. Populates `intrinsics` and
  /// `state` from the provided byte array.
  pub(crate) fn from_bytes( address: SocketAddr, bytes: &[u8] ) -> Self {
    let ( intrinsic_bytes, state_bytes ) = bytes.split_at( ETHERDREAM_INTRINSIC_BYTES );

    // SAFETY: The two uses of unwrap are safe as they are inclusive of the
    // broadcast byte length.
    Self{
      address,
      intrinsics: Intrinsics::from_bytes( intrinsic_bytes ),
      state: State::from_bytes( state_bytes )
    }
  }

  /// Creates a new `Device` from constituent `intrinsics` and `state` parts.
  pub fn from_parts( address: SocketAddr, intrinsics: Intrinsics, state: State ) -> Self {
    Self{ address, intrinsics, state }
  }

  /// Returns the socket address that the device is communicating on.
  #[inline]
  pub fn address( &self ) -> SocketAddr { self.address }

  /// Returns the maximum number of points the device can buffer.
  #[inline]
  pub fn buffer_capacity( &self ) -> u16 { self.intrinsics.buffer_capacity }

  /// Returns the MAC address for this device.
  #[inline]
  pub fn mac_address( &self ) -> MacAddress { self.intrinsics.mac_address }

  /// Returns the maximum number of points this device can process per second.
  #[inline]
  pub fn max_points_per_second( &self ) -> u32 { self.intrinsics.max_points_per_second }

  /// Returns the state of the device when the device was discovered.
  #[inline]
  pub fn state( &self ) -> State { self.state }

  /// Returns the hardware and software version installed on the device.
  #[inline]
  pub fn version( &self ) -> Version { self.intrinsics.version }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Intrinsics

/// A model that describes an Etherdream device's intrinsic properties, as
/// provided by an Etherdream's network broadcast message.
#[derive( Clone, Copy, Debug, Default, PartialEq )]
pub struct Intrinsics {
  /// MAC address of the Etherdream device.
  pub mac_address: MacAddress,

  /// Hardware and software version for the Etherdream device.
  pub version: Version,

  /// Maximum capacity of the Etherdream device point buffer.
  pub buffer_capacity: u16,

  /// Maximum number of points the Etherdream device can process per second.
  pub max_points_per_second: u32,
}

impl Intrinsics {
  /// Decodes an Etherdream network byte array into an `Intrinsic` model.
  pub(crate) fn from_bytes( bytes: &[u8] ) -> Self {
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

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  State

/// A model that describes an Etherdream engine status.
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
  /// Decodes an Etherdream network byte array into a `Status` model.
  pub(crate) fn from_bytes( bytes: &[u8] ) -> Self {
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
  fn from_byte( byte: u8 ) -> Self {
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
  fn from_byte( byte: u8 ) -> Self {
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
  fn from_byte( byte: u8 ) -> Self {
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
  address: [u8; 6]
}

impl MacAddress {
  pub fn new( address: [u8; 6] ) -> Self {
    Self{ address }
  }

  pub fn as_slice( &self ) -> &[u8] {
    &self.address
  }
}

impl Default for MacAddress {
  fn default() -> Self {
    Self{ address: [ 0, 0, 0, 0, 0, 0 ] }
  }
}

impl fmt::Display for MacAddress {
  fn fmt( &self, f: &mut fmt::Formatter ) -> fmt::Result {
    write!( f, "{:02X?}", self.address )
  }
}

impl From<[u8;6]> for MacAddress {
  fn from( bytes: [u8;6] ) -> MacAddress {
    MacAddress::new( bytes )
  }
}