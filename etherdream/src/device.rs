use std::fmt;

pub const DEVICE_BYTES_SIZE: usize = 36;
pub const DEVICE_STATE_BYTES_SIZE: usize = 20;
pub const DEFAULT_PORT: u16 = 7765;

// - - - - - - - - - - - - - - - - - - - - - - - - - - Supporting Enums & Types

#[repr( u8 )]
#[derive( Clone, Copy, Debug, PartialEq )]
pub enum LightEngineState {
  Ready = 0,
  WarmUp = 1,
  CoolDown = 2,
  Estop = 3
}

#[repr( u8 )]
#[derive( Clone, Copy, Debug, PartialEq )]
pub enum PlaybackState {
  Idle = 0,
  Prepared = 1,
  Playing = 2
}

#[repr( u8 )]
#[derive( Clone, Copy, Debug, PartialEq )]
pub enum Source {
  Network = 0,
  Ilda = 1,
  Internal = 2
}

#[repr( C )]
#[derive( Clone, Copy, Debug, PartialEq )]
pub struct Version {
  hardware: u16,
  software: u16
}

impl Version {
  pub fn new( hardware_version: u16, software_version: u16 ) -> Self {
    return Self{ hardware: hardware_version, software: software_version };
  }
}

#[repr( C )]
#[derive( Clone, Copy, Debug, PartialEq )]
pub struct MacAddress {
  address: [u8; 6]
}

impl MacAddress {
  pub fn new( address: [u8; 6] ) -> MacAddress {
    return MacAddress{ address: address };
  }
}

impl From<MacAddress> for [u8; 6] {
  fn from( mac_address: MacAddress ) -> Self {
    return mac_address.address;
  }
}

impl fmt::Display for MacAddress {
  fn fmt( &self, f: &mut fmt::Formatter ) -> fmt::Result {
    return write!( f, "{:02X?}", self.address );
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  State

/// Encodes the Etherdream DAC state, received from any DAC ack/nak.
/// Reference: https://ether-dream.com/protocol.html
#[repr( C )]
#[derive( Clone, Copy, Debug )]
pub struct State {
  // [Unknown] (byte position: native = 0 | from Device = 16)
  pub protocol: u8,

  // The current light engine state for the DAC (bytes = 1|17)
  pub light_engine_state: LightEngineState,

  /// The current playback state for the DAC (bytes = 2|18)
  pub playback_state: PlaybackState,

  // The current playback source for the DAC (bytes = 3|19)
  pub source: Source,

  // The current light engine flags. DAC is ready if all flags are zero. (bytes = 4-5|20-21)
  // Bits:
  //  [0]: Emergency stop occurred due to E-Stop packet or invalid command.
  //  [1]: Emergency stop occurred due to E-Stop input to projector.
  //  [2]: Emergency stop input to projector is currently active.
  //  [3]: Emergency stop occurred due to overtemperature condition.
  //  [4]: Overtemperature condition is currently active.
  //  [5]: Emergency stop occurred due to loss of Ethernet link.
  pub light_engine_flags: u16,

  // The current playback flags. Bits will be non-zero during normal operation. (bytes = 6-7|22-23)
  // Bits:
  //  [0]: Shutter state: 0 = closed, 1 = open.
  //  [1]: Underflow. 1 if the last stream ended with underflow, rather than a 
  //			 Stop command. Reset to zero by the Prepare command.
  //  [2]: E-Stop. 1 if the last stream ended because the E-Stop state was 
  //       entered. Reset to zero by the Prepare command.
  pub playback_flags: PlaybackState,

  // [Unknown] (bytes = 8-9|24-25)
  pub source_flags: u16,

  // The current number of points that are buffered in the DAC. (bytes = 10-11|26-27)
  pub points_buffered: u16,

  // The current number of points that the DAC is processing per second. (bytes = 12-15|28-31)
  pub points_per_second: u32,

  // The total number of points that the DAC has processed. (bytes = 16-19|32-35)
  pub points_lifetime: u32
}

impl State {
  pub fn from_bytes( bytes: [u8; DEVICE_STATE_BYTES_SIZE] ) -> Self {
    // SAFETY:
    // * `State` has the same size as `[u8; DEVICE_STATE_BYTES_SIZE]`.
    // * `[u8; DEVICE_STATE_BYTES_SIZE]` has no alignment requirement.
    // * Since `State` is desgined as packed, this type has no padding.
    unsafe{ return std::mem::transmute( bytes ); }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Device

/// Encodes a device response from an Etherdream DAC discovery broadcast. 
/// Reference: https://ether-dream.com/protocol.html
#[repr( C )]
#[derive( Clone, Copy, Debug )]
pub struct Device {
  // MAC address of the DAC (bytes = 0-5)
  pub mac_address: MacAddress,

  // Hardware and software version of the DAC (bytes = 6-9)
  pub version: Version,

  // Maximum capacity of the DAC data buffer (bytes = 10-11)
  pub buffer_capacity: u16,

  // Maximum number of points the DAC can process per second (bytes = 12-15)
  pub max_points_per_second: u32,

  // The current device state (bytes = 16-36)
  pub state: State
}

impl Device {
  pub fn from_bytes( bytes: [u8; DEVICE_BYTES_SIZE] ) -> Device {
    // SAFETY:
    // * `Device` has the same size as `[u8; DEVICE_BYTES_SIZE]`.
    // * `[u8; DEVICE_BYTES_SIZE]` has no alignment requirement.
    // * Since `Device` is designed as packed, this type has no padding.
    unsafe{ return std::mem::transmute( bytes ); }
  }

  pub fn to_bytes( self ) -> [u8; DEVICE_BYTES_SIZE] {
    // SAFETY:
    // * `Device` has the same size as `[u8; DEVICE_BYTES_SIZE]`.
    // * `[u8; DEVICE_BYTES_SIZE]` has no alignment requirement.
    // * Since `Device` is designed as packed, this type has no padding.
    unsafe{ return std::mem::transmute( self ); }
  }
}

impl fmt::Display for Device {
  fn fmt( &self, f: &mut fmt::Formatter ) -> fmt::Result {
    writeln!( f, "Core properties:" )?;
    writeln!( f, "  Buffer capacity = {}", self.buffer_capacity )?;
    writeln!( f, "  Mac address = {}", self.mac_address )?;
    writeln!( f, "  Max points per second = {}", self.max_points_per_second )?;
    writeln!( f, "  Version = hardware: {}; software: {}", self.version.hardware, self.version.software )?;

    writeln!( f, "\nCurrent state:" )?;
    writeln!( f, "  Light engine = {:?}", self.state.light_engine_state )?;
    writeln!( f, "  Playback = {:?}", self.state.playback_state )?;
    writeln!( f, "  Points lifetime = {:?}", self.state.points_lifetime )?;
    writeln!( f, "  Points per second = {:?}", self.state.points_per_second )?;
    return writeln!( f, "  Source = {:?}", self.state.source );
  }
}