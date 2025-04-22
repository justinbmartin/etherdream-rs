use std::fmt;

use zerocopy::{ FromBytes, Immutable, IntoBytes, transmute };


// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Device

#[repr( u8 )]
#[derive( Debug, PartialEq )]
pub enum LightEngineState {
  Ready = 0,
  WarmUp = 1,
  CoolDown = 2,
  Estop = 3,
  UNKNOWN = u8::MAX
}

#[repr( u8 )]
#[derive( Debug, PartialEq )]
pub enum PlaybackState {
  Idle = 0,
  Prepared = 1,
  Playing = 2,
  UNKNOWN = u8::MAX
}

#[repr( u8 )]
#[derive( Debug, PartialEq )]
pub enum Source {
  Network = 0,
  Ilda = 1,
  Internal = 2,
  UNKNOWN = u8::MAX
}

#[repr( C )]
#[derive( Clone, Copy, Debug, Default, FromBytes, Immutable, IntoBytes, PartialEq )]
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
#[derive( Clone, Copy, Debug, Default, FromBytes, Immutable, IntoBytes, PartialEq )]
pub struct MacAddress {
  address: [u8;6]
}

impl MacAddress {
  pub fn new( address: [u8;6] ) -> MacAddress {
    return MacAddress{ address: address };
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

#[repr( C )]
#[derive( Clone, Copy, Default, FromBytes, Immutable, IntoBytes )]
pub struct Device {
  mac_address: MacAddress,
  version: Version,
  buffer_capacity: u16,
  max_points_per_second: u32,
  state: State
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

#[repr( C )]
#[derive( Clone, Copy, Default, FromBytes, Immutable, IntoBytes )]
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

impl Device {
  pub fn from_bytes( bytes: [u8;36] ) -> Device {
    return transmute!( bytes );
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
      _ => LightEngineState::UNKNOWN
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
      _ => PlaybackState::UNKNOWN
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
      _ => Source::UNKNOWN
    }
  }

  pub fn is_source( &self, source: Source ) -> bool {
    return self.state.source == source as u8;
  }
}


// - - - - - - - - - - - - - - - - - - - - - - - - - - Device Builder (Testing)

#[derive( Clone, Copy )]
pub struct DeviceBuilder {
  inner: Device
}

impl DeviceBuilder {
  pub fn new() -> Self { 
    return Self{
      inner: Device::default()
    };
  }

  pub fn to_device( &self ) -> Device {
    return self.inner;
  }

  pub fn buffer_capacity( &mut self, capacity: u16 ) -> &mut Self {
    self.inner.buffer_capacity = capacity;
    return self;
  }

  pub fn light_engine_state( &mut self, state: LightEngineState ) -> &mut Self {
    self.inner.state.light_engine_state = state as u8;
    return self;
  }

  pub fn mac_address( &mut self, address: [u8;6] ) -> &mut Self {
    self.inner.mac_address.address = address;
    return self;
  }

  pub fn max_points_per_second( &mut self, points_per_second: u32 ) -> &mut Self {
    self.inner.max_points_per_second = points_per_second;
    return self;
  }

  pub fn playback_state( &mut self, state: PlaybackState ) -> &mut Self {
    self.inner.state.playback_state = state as u8;
    return self;
  }

  pub fn points_lifetime( &mut self, count: u32 ) -> &mut Self {
    self.inner.state.points_lifetime = count;
    return self;
  }

  pub fn points_per_second( &mut self, points_per_second: u32 ) -> &mut Self {
    self.inner.state.points_per_second = points_per_second;
    return self;
  }

  pub fn source( &mut self, source: Source ) -> &mut Self {
    self.inner.state.source = source as u8;
    return self;
  }

  pub fn version( &mut self, hardware: u16, software: u16 ) -> &mut Self {
    self.inner.version = Version::new( hardware, software );
    return self;
  }
}