//! Helper model to create Etherdream broadcast device payloads for testing.
use etherdream::constants::*;
use etherdream::device;

#[derive( Clone, Copy )]
pub struct TestDevice {
  pub intrinsics: device::Intrinsics,
  pub state: device::State,
}

impl TestDevice {
  /// Creates a new test device with useful default values for testing.
  pub fn new() -> Self {
    Self::with_capacity( 64 )
  }

  /// Creates a new test device with a custom `capacity`.
  pub fn with_capacity( capacity: u16 ) -> Self {
    Self{
      intrinsics: device::Intrinsics{
        buffer_capacity: capacity,
        mac_address: device::MacAddress::new([ 0, 1, 2, 3, 4, 5 ]),
        max_points_per_second: 128,
        version: device::Version::new( 0, 1 )
      },
      state: device::State{
        protocol: 0,
        light_engine_flags: 0,
        light_engine_state: device::LightEngineState::Ready,
        playback_flags: 0,
        playback_state: device::PlaybackState::Prepared,
        points_buffered: 0,
        points_lifetime: 0,
        points_per_second: 0,
        source: device::Source::Network,
        source_flags: 0
      }
    }
  }
}

/// Copies intrinsic and state attributes into an Etherdream-compliant broadcast byte array.
pub fn copy_into_etherdream_broadcast_bytes( buf: &mut [u8], intrinsics: &device::Intrinsics, state: &device::State ) {
  // Copy intrinsic device properties into `buf`
  buf[0..6].copy_from_slice( intrinsics.mac_address.as_slice() );
  buf[6..8].copy_from_slice( &intrinsics.version.hardware.to_le_bytes() );
  buf[8..10].copy_from_slice( &intrinsics.version.software.to_le_bytes() );
  buf[10..12].copy_from_slice( &intrinsics.buffer_capacity.to_le_bytes() );
  buf[12..16].copy_from_slice( &intrinsics.max_points_per_second.to_le_bytes() );

  copy_into_etherdream_state_bytes( &mut buf[16..ETHERDREAM_BROADCAST_BYTES], &state );
}

/// Copies device state attributes into an Etherdream-compliant state byte array.
pub fn copy_into_etherdream_state_bytes( buf: &mut [u8], state: &device::State ) {
  buf[0] = 0;
  buf[1] = match state.light_engine_state {
    device::LightEngineState::Ready => 0,
    device::LightEngineState::WarmUp => 1,
    device::LightEngineState::CoolDown => 2,
    device::LightEngineState::Estop => 3
  };

  buf[2] = match state.playback_state {
    device::PlaybackState::Idle => 0,
    device::PlaybackState::Prepared => 1,
    device::PlaybackState::Playing => 2
  };

  buf[3] = match state.source {
    device::Source::Network => 0,
    device::Source::Ilda => 1,
    device::Source::Internal => 2,
  };

  buf[4..10].fill( 0 );
  buf[10..12].copy_from_slice( &state.points_buffered.to_le_bytes() );
  buf[12..16].copy_from_slice( &state.points_per_second.to_le_bytes() );
  buf[16..20].copy_from_slice( &state.points_lifetime.to_le_bytes() );
}