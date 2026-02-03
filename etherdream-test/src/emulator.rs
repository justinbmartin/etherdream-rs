use std::io;
use std::net::{ IpAddr, Ipv4Addr, SocketAddr };
use std::sync::{ Arc, RwLock };

use tokio::io::{ AsyncReadExt, AsyncWriteExt };
use tokio::net::TcpListener;
use tokio::task;

use etherdream::DeviceInfo;
use etherdream::protocol;

/// A server that emulates an Etherdream DAC. Useful for software testing.
pub struct Emulator {
  /// The local socket address that the emulator is running on.
  address: SocketAddr,

  /// The intrinsic properties of the device associated with this emulator.
  intrinsics: protocol::Intrinsics,

  /// The real-time device state associated with this emulator.
  state: Arc<RwLock<protocol::State>>,

  /// The join handle that owns the asynchronous server task.
  _handle: task::JoinHandle<io::Result<()>>
}

impl Emulator {
  /// Starts an Etherdream emulation server.
  pub async fn start() -> io::Result<Self> {
    let ( intrinsics, state ) = default_test_device_properties();
    Self::do_start( intrinsics, state ).await
  }

  /// Starts an Etherdream emulation server using the provided point buffer
  /// `capacity`.
  pub async fn start_with_capacity( capacity: usize ) -> io::Result<Self> {
    let ( mut intrinsics, state ) = default_test_device_properties();
    intrinsics.buffer_capacity = capacity as u16;

    Self::do_start( intrinsics, state ).await
  }

  async fn do_start( intrinsics: protocol::Intrinsics, state: protocol::State ) -> io::Result<Self> {
    let state = Arc::new( RwLock::new( state ) );
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
              if let Ok( mut state) = state.write() {
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

          //
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

    Ok( Self{
      address,
      intrinsics,
      state,
      _handle: handle
    })
  }

  /// Returns an `etherdream::DeviceInfo` from the running emulator.
  pub fn device_info( &self ) -> DeviceInfo {
    DeviceInfo::new( self.address, self.intrinsics )
  }

  pub fn point_count( &self ) -> usize {
    if let Ok( state ) = self.state.read() {
      state.points_buffered as usize
    } else {
      1234
    }
  }

  /// Consumes `count` points from emulator's point buffer.
  pub fn consume_points( &mut self, count: u16 ) {
    if let Ok( mut state ) = self.state.write() {
      state.points_buffered = state.points_buffered.saturating_sub( count );
    }
  }
}

fn default_test_device_properties() -> ( protocol::Intrinsics, protocol::State ) {
  let intrinsics = protocol::Intrinsics{
    buffer_capacity: 1024,
    ..Default::default()
  };

  let state = protocol::State{
    playback_state: protocol::PlaybackState::Prepared,
    ..Default::default()
  };

  ( intrinsics, state )
}