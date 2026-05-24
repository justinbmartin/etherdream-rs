//! Simulator: A tokio TCP server that simulates a running Etherdream DAC.
//! Useful for local software development.
//!
//! Developer Note: The complete Etherdream protocol is not fully implemented.
//! Be sure to read and understand this module to understand protocol
//! limitations.
use std::io;
use std::net::{ IpAddr, Ipv4Addr, SocketAddr };
use std::sync::{ Arc, RwLock };
use std::time::Duration;

use tokio::io::{ AsyncReadExt, AsyncWriteExt };
use tokio::net::{ TcpListener, UdpSocket };
use tokio::task;
use tokio::time::interval;

use etherdream::protocol;

const DEFAULT_POINT_BUFFER_CAPACITY: u16 = 1024;

/// A tokio TCP server that implements the Etherdream protocol.
pub struct Simulator {
  // The local socket address that the simulator is communicating on.
  address: SocketAddr,
  // ...
  _broadcast_handle: task::JoinHandle<io::Result<()>>,
  // The intrinsic properties of the device associated with this simulator.
  intrinsics: protocol::Intrinsics,
  // The real-time device state associated with this simulator.
  state: Arc<RwLock<protocol::State>>,
  // The join handle that owns the asynchronous server task.
  handle: task::JoinHandle<io::Result<()>>
}

impl Simulator {
  /// Starts an Etherdream simulator with the default capacity.
  pub async fn start() -> io::Result<Self> {
    Self::start_with_capacity( DEFAULT_POINT_BUFFER_CAPACITY ).await
  }

  /// Starts an Etherdream simulator using the provided point buffer
  /// `capacity`. Will bind locally to `127.0.0.1:*` (any available port).
  pub async fn start_with_capacity( capacity: u16 ) -> io::Result<Self> {
    let intrinsics = protocol::Intrinsics{
      buffer_capacity: capacity,
      ..Default::default()
    };

    let state = Arc::new( RwLock::new( protocol::State{
      playback_state: protocol::PlaybackState::Prepared,
      ..Default::default()
    } ) );

    // Start listening on `127.0.0.1:*` (any available port)
    let listener = TcpListener::bind( SocketAddr::new( IpAddr::V4( Ipv4Addr::LOCALHOST ), 0 ) ).await?;
    let address = listener.local_addr()?;

    let handle = tokio::spawn({
      let state = state.clone();

      async move {
        let mut cmd: u8;
        let mut control_signal: u8;
        let mut tx_buf = [0u8; protocol::RESPONSE_BYTES_SIZE];

        // Create our network receive buffer. Initialize the buffer with enough
        // space to contain the maximum amount of point data that the device
        // intrinsics define. Three bytes are added for point data header info.
        let max_rx_buffer_size = ( intrinsics.buffer_capacity as usize * protocol::POINT_DATA_BYTES_SIZE ) + 3;
        let mut rx_buf = vec![0u8; max_rx_buffer_size];

        // Start listening
        let ( mut stream, _remote ) = listener.accept().await?;

        loop {
          // Extract the command byte and reset our control signal
          cmd = stream.read_u8().await?;
          control_signal = protocol::CONTROL_ACK;

          match cmd {
            protocol::COMMAND_PREPARE => {
              if let Ok( mut state ) = state.write() {
                state.light_engine_state = protocol::LightEngineState::Ready;
                state.playback_state = protocol::PlaybackState::Prepared;
                state.points_buffered = 0;
              }
            },
            protocol::COMMAND_DATA => {
              // Read the point data into `rx_buf`.
              let point_count = stream.read_u16_le().await?;
              stream.read_exact( &mut rx_buf[..( point_count as usize * protocol::POINT_DATA_BYTES_SIZE )] ).await?;

              if let Ok( mut state ) = state.write() {
                if point_count <= intrinsics.buffer_capacity.saturating_sub( state.points_buffered ) {
                  state.points_buffered += point_count;
                } else {
                  control_signal = protocol::CONTROL_NAK_FULL;
                }
              }
            },
            protocol::COMMAND_BEGIN => {
              let _unused = stream.read_u16_le().await?;
              let queue_rate = stream.read_u32_le().await?;

              if let Ok( mut state ) = state.write() {
                state.points_per_second = queue_rate;
                state.playback_state = protocol::PlaybackState::Playing;
              }
            },
            protocol::COMMAND_STOP => {
              if let Ok( mut state ) = state.write() {
                if state.is_ready() {
                  state.playback_state = protocol::PlaybackState::Idle;
                } else {
                  control_signal = protocol::CONTROL_NAK_INVALID;
                }
              }
            },
            protocol::COMMAND_CLEAR | protocol::COMMAND_PING => {
              /* no-op */
            },
            _unknown_cmd => {
              control_signal = protocol::CONTROL_NAK_INVALID;
            }
          };

          // Send the Etherdream response
          if let Ok( state ) = state.read() {
            tx_buf[0] = control_signal;
            tx_buf[1] = cmd;
            tx_buf[2] = 0;
            tx_buf[3] = match state.light_engine_state {
              protocol::LightEngineState::Ready => 0,
              protocol::LightEngineState::WarmUp => 1,
              protocol::LightEngineState::CoolDown => 2,
              protocol::LightEngineState::Estop => 3
            };

            tx_buf[4] = match state.playback_state {
              protocol::PlaybackState::Idle => 0,
              protocol::PlaybackState::Prepared => 1,
              protocol::PlaybackState::Playing => 2
            };

            tx_buf[5] = match state.source {
              protocol::Source::Network => 0,
              protocol::Source::Ilda => 1,
              protocol::Source::Internal => 2,
            };

            tx_buf[6..12].fill( 0 );
            tx_buf[12..14].copy_from_slice( &state.points_buffered.to_le_bytes() );
            tx_buf[14..18].copy_from_slice( &state.points_per_second.to_le_bytes() );
            tx_buf[18..22].copy_from_slice( &state.points_lifetime.to_le_bytes() );
          }

          let _ = stream.write( &tx_buf ).await?;
        }
      }
    });

    let broadcast_handle = tokio::spawn({
      let socket = UdpSocket::bind( SocketAddr::new( IpAddr::V4( Ipv4Addr::LOCALHOST ), 0 ) ).await?;
      socket.set_broadcast( true )?;

      let broadcast_addr = SocketAddr::new( IpAddr::V4( Ipv4Addr::LOCALHOST ), protocol::BROADCAST_PORT );
      let mut interval = interval( Duration::from_secs( 1 ) );

      let intrinsics = intrinsics.clone();
      let state = state.clone();
      let mut tx_buf = [0u8; protocol::BROADCAST_BYTES_SIZE];

      async move {
        loop {
          interval.tick().await;

          tx_buf[0..6].copy_from_slice( intrinsics.mac_address.as_slice() );
          tx_buf[6..8].copy_from_slice( &intrinsics.version.hardware.to_le_bytes() );
          tx_buf[8..10].copy_from_slice( &intrinsics.version.software.to_le_bytes() );
          tx_buf[10..12].copy_from_slice( &intrinsics.buffer_capacity.to_le_bytes() );
          tx_buf[12..16].copy_from_slice( &intrinsics.max_points_per_second.to_le_bytes() );

          if let Ok( state ) = state.read() {
            tx_buf[16] = 0;
            tx_buf[17] = match state.light_engine_state {
              protocol::LightEngineState::Ready => 0,
              protocol::LightEngineState::WarmUp => 1,
              protocol::LightEngineState::CoolDown => 2,
              protocol::LightEngineState::Estop => 3
            };

            tx_buf[18] = match state.playback_state {
              protocol::PlaybackState::Idle => 0,
              protocol::PlaybackState::Prepared => 1,
              protocol::PlaybackState::Playing => 2
            };

            tx_buf[19] = match state.source {
              protocol::Source::Network => 0,
              protocol::Source::Ilda => 1,
              protocol::Source::Internal => 2,
            };

            tx_buf[20..26].fill( 0 );
            tx_buf[26..28].copy_from_slice( &state.points_buffered.to_le_bytes() );
            tx_buf[28..32].copy_from_slice( &state.points_per_second.to_le_bytes() );
            tx_buf[32..36].copy_from_slice( &state.points_lifetime.to_le_bytes() );
          }

          socket.send_to( &tx_buf, broadcast_addr ).await?;
        }
      }
    });

    Ok( Self{
      address,
      _broadcast_handle: broadcast_handle,
      handle,
      intrinsics,
      state
    })
  }

  /// Returns the address that this simulator is running on.
  pub fn address( &self ) -> &SocketAddr {
    &self.address
  }

  /// Returns the intrinsic device properties of this simulator.
  pub fn intrinsics( &self ) -> &protocol::Intrinsics {
    &self.intrinsics
  }

  /// Returns the point count currently buffered in the simulator.
  pub fn point_count( &self ) -> usize {
    if let Ok( state ) = self.state.read() {
      state.points_buffered as usize
    } else {
      0
    }
  }

  /// Consumes `count` points from simulator's point buffer.
  pub fn consume_points( &mut self, count: u16 ) {
    if let Ok( mut state ) = self.state.write() {
      state.points_buffered = state.points_buffered.saturating_sub( count );
    }
  }

  pub async fn stop( self ) {
    let _ = self.handle.await;
  }
}