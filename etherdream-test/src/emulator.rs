//! A local server that emulates an Etherdream DAC for testing.
use std::io;
use std::net::{ IpAddr, Ipv4Addr, SocketAddr };
use std::sync::{ Arc, RwLock };

use tokio::io::{ AsyncReadExt, AsyncWriteExt };
use tokio::net::TcpListener;
use tokio::task;

use etherdream::constants::*;
use etherdream::device::{ Device, Intrinsics, LightEngineState, PlaybackState, Source, State };

pub struct Emulator {
  /// The local socket address that the emulator is running on.
  address: SocketAddr,

  /// The intrinsic properties of the device associated with this emulator.
  intrinsics: Intrinsics,

  /// The real-time device state associated with this emulator.
  state: Arc<RwLock<State>>,

  /// The join handle that owns the asynchronous server task.
  _handle: task::JoinHandle<io::Result<()>>
}

impl Emulator {
  /// Starts an Etherdream emulation server using the default test device
  /// configuration.
  pub async fn start() -> io::Result<Self> {
    let ( intrinsics, state ) = default_test_device_properties();
    Self::do_start( intrinsics, state ).await
  }

  /// Starts an emulation server using the provided point buffer `capacity` on
  /// any available port using the default test device configuration.
  pub async fn start_with_capacity( capacity: usize ) -> io::Result<Self> {
    let ( mut intrinsics, state ) = default_test_device_properties();
    intrinsics.buffer_capacity = capacity as u16;

    Self::do_start( intrinsics, state ).await
  }

  async fn do_start( intrinsics: Intrinsics, state: State ) -> io::Result<Self> {
    let state = Arc::new( RwLock::new( state ) );
    let listener = TcpListener::bind( SocketAddr::new( IpAddr::V4( Ipv4Addr::LOCALHOST ), 0 ) ).await?;
    let address = listener.local_addr()?;

    let handle = tokio::spawn({
      let state = state.clone();

      async move {
        let mut tx_buf = [0u8; ETHERDREAM_RESPONSE_BYTES];
        let mut cmd: u8;
        let mut control_signal: u8;
        let max_read_buffer_size = ( intrinsics.buffer_capacity as usize * ETHERDREAM_POINT_DATA_BYTES ) + 3;
        let mut rx_buf = vec![0u8; max_read_buffer_size];

        let ( mut stream, _remote ) = listener.accept().await?;

        loop {
          // Extract the first byte as the command
          cmd = stream.read_u8().await?;
          control_signal = ETHERDREAM_CONTROL_ACK;

          match cmd {
            ETHERDREAM_COMMAND_PREPARE => {
              if let Ok( mut state) = state.write() {
                state.light_engine_state = LightEngineState::Ready;
                state.playback_state = PlaybackState::Prepared;
                state.points_buffered = 0;
              }
            },
            ETHERDREAM_COMMAND_DATA => {
              // Read the point data into `rx_buf`.
              let point_count = stream.read_u16_le().await?;
              let point_buffer = &mut rx_buf[..( point_count as usize * ETHERDREAM_POINT_DATA_BYTES )];
              stream.read_exact( point_buffer ).await?;

              if let Ok( mut state ) = state.write() {
                if point_count <= intrinsics.buffer_capacity.saturating_sub( state.points_buffered ) {
                  state.points_buffered += point_count;
                } else {
                  control_signal = ETHERDREAM_CONTROL_NAK_FULL;
                }
              }
            },
            ETHERDREAM_COMMAND_BEGIN => {
              let _unused = stream.read_u16_le().await?;
              let queue_rate = stream.read_u32_le().await?;

              if let Ok( mut state ) = state.write() {
                state.points_per_second = queue_rate;
                state.playback_state = PlaybackState::Playing;
              }
            },
            ETHERDREAM_COMMAND_STOP => {
              if let Ok( mut state ) = state.write() {
                if state.is_ready() {
                  state.playback_state = PlaybackState::Idle;
                } else {
                  control_signal = ETHERDREAM_CONTROL_NAK_INVALID;
                }
              }
            },
            ETHERDREAM_COMMAND_CLEAR | ETHERDREAM_COMMAND_PING => {
              /* no-op */
            },
            _unknown_cmd => {
              control_signal = ETHERDREAM_CONTROL_NAK_INVALID;
            }
          };

          //
          if let Ok( state ) = state.read() {
            tx_buf[0] = control_signal;
            tx_buf[1] = cmd;
            tx_buf[2] = 0;
            tx_buf[3] = match state.light_engine_state {
              LightEngineState::Ready => 0,
              LightEngineState::WarmUp => 1,
              LightEngineState::CoolDown => 2,
              LightEngineState::Estop => 3
            };

            tx_buf[4] = match state.playback_state {
              PlaybackState::Idle => 0,
              PlaybackState::Prepared => 1,
              PlaybackState::Playing => 2
            };

            tx_buf[5] = match state.source {
              Source::Network => 0,
              Source::Ilda => 1,
              Source::Internal => 2,
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

  /// Returns an `etherdream::Device` from the running emulator.
  pub fn device( &self ) -> Option<Device> {
    if let Ok( state ) = self.state.read() {
      Some( Device::from_parts( self.address, self.intrinsics, *state ) )
    } else {
      None
    }
  }

  /// Consumes `count` points from emulator's point buffer.
  pub fn consume_points( &mut self, count: u16 ) {
    if let Ok( mut state ) = self.state.write() {
      state.points_buffered = state.points_buffered.saturating_sub( count );
    }
  }
}

fn default_test_device_properties() -> ( Intrinsics, State ) {
  let intrinsics = Intrinsics{
    buffer_capacity: 1024,
    ..Default::default()
  };

  let state = State{
    playback_state: PlaybackState::Prepared,
    ..Default::default()
  };

  ( intrinsics, state )
}