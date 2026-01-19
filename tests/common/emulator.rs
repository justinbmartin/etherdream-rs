//! A local server that emulates an Etherdream DAC for testing.
use std::io;
use std::net::{ IpAddr, Ipv4Addr, SocketAddr };
use std::sync::{ Arc, RwLock };

use tokio::io::{ AsyncReadExt, AsyncWriteExt };
use tokio::net;
use tokio::task;

use etherdream::constants::*;
use etherdream::device;

pub struct Server {
  /// The local socket address that the emulator is running on.
  address: SocketAddr,

  /// The intrinsic properties of the device associated with this emulator.
  intrinsics: device::Intrinsics,

  /// The real-time device state associated with this emulator.
  state: Arc<RwLock<device::State>>,

  /// The join handle that owns the asynchronous server task.
  _handle: task::JoinHandle<io::Result<()>>
}

impl Server {
  /// Starts an `EtherdreamEmulator` server on any available port using the
  /// default test device configuration.
  pub async fn start() -> io::Result<Self> {
    Self::start_with_device( TestDevice::new() ).await
  }

  /// Starts an `EtherdreamEmulator` server on any available port using the
  /// device configuration provided via `device`.
  pub async fn start_with_device( device: TestDevice ) -> io::Result<Self> {
    let state = Arc::new( RwLock::new( device.state ) );

    let listener = net::TcpListener::bind( SocketAddr::new( IpAddr::V4( Ipv4Addr::LOCALHOST ), 0 ) ).await?;
    let address = listener.local_addr()?;

    let handle = tokio::spawn({
      let state = state.clone();

      async move {
        // The network buffers used to read and write to the connected `etherdream::Client`.
        let mut rx_buf = [0u8; 512];
        let mut tx_buf = [0u8; ETHERDREAM_RESPONSE_BYTES];

        let ( mut stream, _remote ) = listener.accept().await?;

        loop {
          let _ = stream.read( &mut rx_buf ).await?;

          // Extract the first byte and handle it as the command signal.
          match rx_buf[0] {
            cmd @ ( ETHERDREAM_COMMAND_PING | ETHERDREAM_COMMAND_CLEAR ) => {
              ack( &mut tx_buf, cmd, &*state.write().unwrap() )
            },
            ETHERDREAM_COMMAND_PREPARE => {
              let mut state = state.write().unwrap();
              state.points_buffered = 0;
              ack( &mut tx_buf, ETHERDREAM_COMMAND_PREPARE, &state )
            },
            ETHERDREAM_COMMAND_DATA => {
              let mut state = state.write().unwrap();
              let point_count = u16::from_le_bytes( rx_buf[1..3].try_into().unwrap() );
              let points_buffered = state.points_buffered;

              if point_count <= device.intrinsics.buffer_capacity.saturating_sub( points_buffered ) {
                state.points_buffered = points_buffered + point_count;
                ack( &mut tx_buf, ETHERDREAM_COMMAND_DATA, &state )
              } else {
                response( &mut tx_buf, ETHERDREAM_CONTROL_NAK_FULL, ETHERDREAM_COMMAND_DATA, &state )
              }
            },
            ETHERDREAM_COMMAND_BEGIN => {
              let mut state = state.write().unwrap();
              state.points_per_second = u32::from_le_bytes( rx_buf[3..7].try_into().unwrap() );
              state.playback_state = device::PlaybackState::Playing;

              ack( &mut tx_buf, ETHERDREAM_COMMAND_BEGIN, &state )
            },
            ETHERDREAM_COMMAND_STOP => {
              let mut state = state.write().unwrap();

              if state.is_ready() {
                state.playback_state = device::PlaybackState::Idle;
                ack( &mut tx_buf, ETHERDREAM_COMMAND_STOP, &state )
              } else {
                response( &mut tx_buf, ETHERDREAM_CONTROL_NAK_INVALID, ETHERDREAM_COMMAND_STOP, &state )
              }
            },
            unknown_cmd => {
              let state = state.write().unwrap();
              response( &mut tx_buf, ETHERDREAM_CONTROL_NAK_INVALID, unknown_cmd, &state )
            }
          };

          let _ = stream.write( &tx_buf ).await?;
        }
      }
    });

    Ok( Self{
      address,
      intrinsics: device.intrinsics,
      state,
      _handle: handle
    })
  }

  /// Returns an etherdream::Device from the running emulator.
  pub fn device( &self ) -> device::Device {
    device::Device::from_parts( self.address, self.intrinsics, *self.state.read().unwrap() )
  }

  /// Consumes `count` points from emulator.
  pub fn consume_points( &mut self, count: u16 ) {
    let mut guard = self.state.write().unwrap();
    guard.points_buffered = guard.points_buffered.saturating_sub( count );
  }
}

fn ack( buf: &mut [u8], command: u8, state: &device::State ) {
  response( buf, b'a', command, state )
}

fn response( buf: &mut [u8], control_signal: u8, command: u8, state: &device::State ) {
  buf.fill( 0 );

  buf[0] = control_signal;
  buf[1] = command;

  copy_into_etherdream_state_bytes( &mut buf[2..ETHERDREAM_RESPONSE_BYTES], &state );
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Test Device

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