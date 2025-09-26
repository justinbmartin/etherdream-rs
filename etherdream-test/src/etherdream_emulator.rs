//! A local server that emulates an Etherdream DAC for testing.
use std::io;
use std::net::{ IpAddr, Ipv4Addr, SocketAddr };
use std::sync::{ Arc, RwLock };

use tokio::io::{ AsyncReadExt, AsyncWriteExt };
use tokio::net;
use tokio::task;

use etherdream::{ constants::*, device };

use crate::test_device::*;

pub struct EtherdreamEmulator {
  /// The socket address that the emulator is running on.
  address: SocketAddr,

  /// The intrinsic properties of the device associated with this emulator.
  intrinsics: device::Intrinsics,

  /// The real-time device state associated with this emulator.
  state: Arc<RwLock<device::State>>,

  /// The join handle that owns the asynchronous server task.
  _handle: task::JoinHandle<io::Result<()>>
}

impl EtherdreamEmulator {
  /// Starts an `EtherdreamEmulator` server on any available port.
  pub async fn start( device: TestDevice ) -> io::Result<Self> {
    let state = Arc::new( RwLock::new( device.state ) );

    let listener = net::TcpListener::bind( SocketAddr::new( IpAddr::V4( Ipv4Addr::LOCALHOST ), 0 ) ).await?;
    let address = listener.local_addr()?;

    let handle = tokio::spawn({
      let state = state.clone();

      async move {
        // The buffers used to receive and transmit data for the associated
        // `etherdream::Client`.
        let mut client_rx_buf = [0u8; 512];
        let mut client_tx_buf = [0u8; 22];

        let ( mut stream, _remote ) = listener.accept().await?;

        loop {
          let _ = stream.read( &mut client_rx_buf ).await?;

          // Extract the first byte and handle it as the command signal.
          match client_rx_buf[0] {
            cmd @ ( ETHERDREAM_COMMAND_PING | ETHERDREAM_COMMAND_CLEAR ) => {
              let state = state.write().unwrap();
              ack( &mut client_tx_buf, cmd, &state )
            },
            ETHERDREAM_COMMAND_PREPARE => {
              let mut state = state.write().unwrap();
              state.points_buffered = 0;
              ack( &mut client_tx_buf, ETHERDREAM_COMMAND_PREPARE, &state )
            },
            ETHERDREAM_COMMAND_DATA => {
              let mut state = state.write().unwrap();
              let point_count = u16::from_le_bytes( client_rx_buf[1..3].try_into().unwrap() );
              let points_buffered = state.points_buffered;

              if point_count <= device.intrinsics.buffer_capacity.saturating_sub( points_buffered ) {
                state.points_buffered = points_buffered + point_count;
                ack( &mut client_tx_buf, ETHERDREAM_COMMAND_DATA, &state )
              } else {
                response( &mut client_tx_buf, ETHERDREAM_CONTROL_NAK_FULL, ETHERDREAM_COMMAND_DATA, &state )
              }
            },
            ETHERDREAM_COMMAND_BEGIN => {
              let mut state = state.write().unwrap();
              state.points_per_second = u32::from_le_bytes( client_rx_buf[3..7].try_into().unwrap() );
              state.playback_state = device::PlaybackState::Playing;

              ack( &mut client_tx_buf, ETHERDREAM_COMMAND_BEGIN, &state )
            },
            ETHERDREAM_COMMAND_STOP => {
              let mut state = state.write().unwrap();

              if state.is_ready() {
                state.playback_state = device::PlaybackState::Idle;
                ack( &mut client_tx_buf, ETHERDREAM_COMMAND_STOP, &state )
              } else {
                response( &mut client_tx_buf, ETHERDREAM_CONTROL_NAK_INVALID, ETHERDREAM_COMMAND_STOP, &state )
              }
            },
            unknown_cmd => {
              let state = state.write().unwrap();
              response( &mut client_tx_buf, ETHERDREAM_CONTROL_NAK_INVALID, unknown_cmd, &state )
            }
          };

          let _ = stream.write( &client_tx_buf ).await?;
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

  pub fn device( &self ) -> device::Device {
    device::Device::from_parts( self.address, self.intrinsics, *self.state.read().unwrap() )
  }

  pub fn consume_points( &mut self, count: u16 ) {
    let mut guard = self.state.write().unwrap();
    guard.points_buffered = guard.points_buffered.saturating_sub( count );
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - Etherdream Test DAC Response

fn ack( buf: &mut [u8], command: u8, state: &device::State ) {
  response( buf, b'a', command, state )
}

fn response( buf: &mut [u8], control_signal: u8, command: u8, state: &device::State ) {
  buf.fill( 0 );

  buf[0] = control_signal;
  buf[1] = command;

  copy_into_etherdream_state_bytes( &mut buf[2..ETHERDREAM_RESPONSE_BYTES], &state );
}