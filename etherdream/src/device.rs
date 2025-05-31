use std::fmt;

pub const DEVICE_BYTES_SIZE: usize = 36;
pub const DEVICE_STATE_BYTES_SIZE: usize = 20;
pub const DEFAULT_PORT: u16 = 7765;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Enum & Types

#[repr( u8 )]
#[derive( Debug, PartialEq )]
pub enum LightEngineState {
  Ready = 0,
  WarmUp = 1,
  CoolDown = 2,
  Estop = 3
}

#[repr( u8 )]
#[derive( Debug, PartialEq )]
pub enum PlaybackState {
  Idle = 0,
  Prepared = 1,
  Playing = 2
}

#[repr( u8 )]
#[derive( Debug, PartialEq )]
pub enum Source {
  Network = 0,
  Ilda = 1,
  Internal = 2
}

#[repr( C )]
#[derive( Clone, Copy, Debug, Default, PartialEq )]
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
#[derive( Clone, Copy, Debug, Default, PartialEq )]
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
    let mac_address_in_hex = 
      self.address.iter()
      .map( |b| format!( "{:02x}", b ).to_string() )
      .collect::<Vec<String>>()
      .join( "::" );

    return write!( f, "{}", mac_address_in_hex );
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  State

#[repr( C )]
#[derive( Clone, Copy, Default )]
pub struct State {
  // Unknown
  _protocol: u8,

  // 0: Ready
  // 1: Warm-up
  // 2: Cool-down
  // 3: E-Stop
  light_engine_state: u8,

  // 0: Idle
  // 1: Prepared
  // 2: Currently playing
  playback_state: u8,

  // The current playback source for the DAC:
  // 0: Network streaming (default).
  // 1: ILDA playback from SD card.
  // 2: Internal abstract generator.
  source: u8,

  // Light engine is ready if all flags are set to zero. Else, each bit:
  // [0]: Emergency stop occurred due to E-Stop packet or invalid command.
  // [1]: Emergency stop occurred due to E-Stop input to projector.
  // [2]: Emergency stop input to projector is currently active.
  // [3]: Emergency stop occurred due to overtemperature condition.
  // [4]: Overtemperature condition is currently active.
  // [5]: Emergency stop occurred due to loss of Ethernet link.
  _light_engine_flags: u16,

  // Bits will be non-zero during normal operation:
  // [0]: Shutter state: 0 = closed, 1 = open.
  // [1]: Underflow. 1 if the last stream ended with underflow, rather than a 
  //			Stop command. Reset to zero by the Prepare command.
  // [2]: E-Stop. 1 if the last stream ended because the E-Stop state was 
  //      entered. Reset to zero by the Prepare command.
  _playback_flags: u16,

  // Unknown
  _source_flags: u16,

  // Remaining number of points buffered in the DAC
  _points_buffered: u16,

  // Number of points the DAC is processing per second
  points_per_second: u32,

  // The total number of points the DAC has processed
  points_lifetime: u32
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

#[repr( C )]
#[derive( Clone, Copy, Default )]
pub struct Device {
  mac_address: MacAddress,
  version: Version,
  buffer_capacity: u16,
  max_points_per_second: u32,
  state: State
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

  pub fn buffer_capacity( &self ) -> u16 {
    return self.buffer_capacity;
  }

  pub fn mac_address( &self ) -> MacAddress {
    return self.mac_address;
  }

  pub fn max_points_per_second( &self ) -> u32 {
    return self.max_points_per_second;
  }

  pub fn points_lifetime( &self ) -> u32 {
    return self.state.points_lifetime;
  }

  pub fn points_per_second( &self ) -> u32 {
    return self.state.points_per_second;
  }
 
  pub fn version( &self ) -> Version {
    return self.version;
  }

  pub fn light_engine_state( &self ) -> LightEngineState {
    match self.state.light_engine_state {
      0 => LightEngineState::Ready,
      1 => LightEngineState::WarmUp,
      2 => LightEngineState::CoolDown,
      3 => LightEngineState::Estop,
      _ => LightEngineState::Ready // TODO: how to handle incorrect values? they should not exist per contract w/ DAC? panic?
    }
  }

  pub fn is_light_engine_state( &self, state: LightEngineState ) -> bool {
    return self.state.light_engine_state == state as u8;
  }

  pub fn playback_state( &self ) -> PlaybackState {
    match self.state.playback_state {
      0 => PlaybackState::Idle,
      1 => PlaybackState::Prepared,
      2 => PlaybackState::Playing,
      _ => PlaybackState::Idle
    }
  }

  pub fn is_playback_state( &self, state: PlaybackState ) -> bool {
    return self.state.playback_state == state as u8;
  }

  pub fn source( &self ) -> Source {
    match self.state.source {
      0 => Source::Network,
      1 => Source::Ilda,
      2 => Source::Internal,
      _ => Source::Network
    }
  }

  pub fn is_source( &self, source: Source ) -> bool {
    return self.state.source == source as u8;
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
    writeln!( f, "  Light engine = {:?}", self.light_engine_state() )?;
    writeln!( f, "  Playback = {:?}", self.playback_state() )?;
    writeln!( f, "  Points lifetime = {:?}", self.state.points_lifetime )?;
    writeln!( f, "  Points per second = {:?}", self.state.points_per_second )?;
    return writeln!( f, "  Source = {:?}", self.source() );
  }
}