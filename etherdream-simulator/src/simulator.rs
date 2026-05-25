//! Simulator: A tokio TCP server that simulates a running Etherdream DAC.
//! Useful for local software development.
use std::io;
use std::net::{ IpAddr, Ipv4Addr, SocketAddr };
use std::sync::{ Arc, RwLock };
use std::time::Duration;

use tokio::io::{ AsyncReadExt, AsyncWriteExt };
use tokio::net::{ TcpListener, UdpSocket };
use tokio::task;
use tokio::time;
use tokio_util::sync::CancellationToken;

use etherdream::protocol;

const DEFAULT_POINT_BUFFER_CAPACITY: u16 = 1024;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Builder

pub struct Builder {
  address: SocketAddr,
  capacity: u16
}

impl Builder {
  /// Starts an Etherdream simulator with the default capacity.
  pub fn new() -> Self {
    Self{
      capacity: DEFAULT_POINT_BUFFER_CAPACITY,
      address: SocketAddr::new( IpAddr::V4( Ipv4Addr::LOCALHOST ), 0 )
    }
  }

  pub fn address( mut self, address: SocketAddr ) -> Self {
    self.address = address;
    self
  }

  pub fn capacity( mut self, capacity: u16 ) -> Self {
    self.capacity = capacity;
    self
  }

  /// Starts an Etherdream simulator using the provided point buffer
  /// `capacity`. Will bind locally to `127.0.0.1:*` (any available port).
  pub async fn start( self ) -> io::Result<Simulator> {
    let cancellation_token = CancellationToken::new();

    let intrinsics = protocol::Intrinsics{
      buffer_capacity: self.capacity,
      ..Default::default()
    };

    let state = Arc::new( RwLock::new( protocol::State{
      playback_state: protocol::PlaybackState::Prepared,
      ..Default::default()
    } ) );

    // ...
    let api_listener = TcpListener::bind( self.address ).await?;
    let address = api_listener.local_addr()?;

    let handle = tokio::spawn({
      let cancellation_token = cancellation_token.clone();

      let api_service = ApiService{
        intrinsics: intrinsics.clone(),
        listener: api_listener,
        state: state.clone()
      };

      let broadcast_service = BroadcastService{
        ip_addr: self.address.ip(),
        intrinsics: intrinsics.clone(),
        state: state.clone()
      };

      async move {
        tokio::select!{
          _ = cancellation_token.cancelled() => Ok( () ),
          result = api_service.run() => result,
          result = broadcast_service.run() => result
        }
      }
    });

    Ok( Simulator{
      address,
      cancellation_token,
      handle,
      intrinsics,
      state
    })
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Simulator

/// A tokio TCP server that implements the Etherdream protocol.
pub struct Simulator {
  // The local socket address that the simulator is communicating on.
  address: SocketAddr,
  // The cancellation token used to shut down the simulator.
  cancellation_token: CancellationToken,
  // The intrinsic properties of the device associated with this simulator.
  intrinsics: protocol::Intrinsics,
  // The real-time device state associated with this simulator.
  state: Arc<RwLock<protocol::State>>,
  // The join handle that owns the asynchronous server tasks.
  handle: task::JoinHandle<io::Result<()>>
}

impl Simulator {
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
    self.cancellation_token.cancel();
    let _ = self.handle.await;
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  API Service

struct ApiService {
  intrinsics: protocol::Intrinsics,
  listener: TcpListener,
  state: Arc<RwLock<protocol::State>>
}

impl ApiService {
  async fn run( &self ) -> io::Result<()> {
    let mut cmd: u8;
    let mut control_signal: u8;
    let mut tx_buf = [0u8; protocol::RESPONSE_BYTES_SIZE];

    // Create our network receive buffer. Initialize the buffer with enough
    // space to contain the maximum amount of point data that the device
    // intrinsics define. Three bytes are added for point data header info.
    let max_rx_buffer_size = ( self.intrinsics.buffer_capacity as usize * protocol::POINT_DATA_BYTES_SIZE ) + 3;
    let mut rx_buf = vec![0u8; max_rx_buffer_size];

    // Start listening
    let ( mut stream, _remote ) = self.listener.accept().await?;

    loop {
      // Extract the command byte and reset our control signal
      cmd = stream.read_u8().await?;
      control_signal = protocol::CONTROL_ACK;

      match cmd {
        protocol::COMMAND_PREPARE => {
          if let Ok( mut state ) = self.state.write() {
            state.light_engine_state = protocol::LightEngineState::Ready;
            state.playback_state = protocol::PlaybackState::Prepared;
            state.points_buffered = 0;
          }
        },
        protocol::COMMAND_DATA => {
          // Read the point data into `rx_buf`.
          let point_count = stream.read_u16_le().await?;
          stream.read_exact( &mut rx_buf[..( point_count as usize * protocol::POINT_DATA_BYTES_SIZE )] ).await?;

          if let Ok( mut state ) = self.state.write() {
            if point_count <= self.intrinsics.buffer_capacity.saturating_sub( state.points_buffered ) {
              state.points_buffered += point_count;
            } else {
              control_signal = protocol::CONTROL_NAK_FULL;
            }
          }
        },
        protocol::COMMAND_BEGIN => {
          let _unused = stream.read_u16_le().await?;
          let queue_rate = stream.read_u32_le().await?;

          if let Ok( mut state ) = self.state.write() {
            state.points_per_second = queue_rate;
            state.playback_state = protocol::PlaybackState::Playing;
          }
        },
        protocol::COMMAND_STOP => {
          if let Ok( mut state ) = self.state.write() {
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

      tx_buf[0] = control_signal;
      tx_buf[1] = cmd;
      copy_state_into_buf( &mut tx_buf[2..], &self.state );

      // Send the Etherdream response
      let _ = stream.write( &tx_buf ).await?;
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Broadcast Service

struct BroadcastService {
  intrinsics: protocol::Intrinsics,
  ip_addr: IpAddr,
  state: Arc<RwLock<protocol::State>>
}

impl BroadcastService {
  async fn run( &self ) -> io::Result<()> {
    let socket = UdpSocket::bind( SocketAddr::new( self.ip_addr, 0 ) ).await?;
    socket.set_broadcast( true )?;

    let broadcast_addr = SocketAddr::new( self.ip_addr, protocol::BROADCAST_PORT );
    let mut interval = time::interval( Duration::from_secs( 1 ) );

    let mut buf = [0u8; protocol::BROADCAST_BYTES_SIZE];

    loop {
      interval.tick().await;

      buf[0..6].copy_from_slice( self.intrinsics.mac_address.as_slice() );
      buf[6..8].copy_from_slice( &self.intrinsics.version.hardware.to_le_bytes() );
      buf[8..10].copy_from_slice( &self.intrinsics.version.software.to_le_bytes() );
      buf[10..12].copy_from_slice( &self.intrinsics.buffer_capacity.to_le_bytes() );
      buf[12..16].copy_from_slice( &self.intrinsics.max_points_per_second.to_le_bytes() );
      copy_state_into_buf( &mut buf[16..], &self.state );

      socket.send_to( &buf, broadcast_addr ).await?;
    }
  }
}

fn copy_state_into_buf( buf: &mut [u8], state: &Arc<RwLock<protocol::State>> ) {
  if let Ok( state ) = state.read() {
    buf[0] = 0;
    buf[1] = match state.light_engine_state {
      protocol::LightEngineState::Ready => 0,
      protocol::LightEngineState::WarmUp => 1,
      protocol::LightEngineState::CoolDown => 2,
      protocol::LightEngineState::Estop => 3
    };

    buf[2] = match state.playback_state {
      protocol::PlaybackState::Idle => 0,
      protocol::PlaybackState::Prepared => 1,
      protocol::PlaybackState::Playing => 2
    };

    buf[3] = match state.source {
      protocol::Source::Network => 0,
      protocol::Source::Ilda => 1,
      protocol::Source::Internal => 2,
    };

    buf[4..10].fill( 0 );
    buf[10..12].copy_from_slice( &state.points_buffered.to_le_bytes() );
    buf[12..16].copy_from_slice( &state.points_per_second.to_le_bytes() );
    buf[16..20].copy_from_slice( &state.points_lifetime.to_le_bytes() );
  }
}