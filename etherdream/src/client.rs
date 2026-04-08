use std::fmt;
use std::io;
use std::net::SocketAddr;
use std::sync::{ Arc, atomic::{ AtomicBool, Ordering::* } };
use std::time::{ Duration, Instant };

use tokio::io::{ AsyncReadExt, AsyncWriteExt };
use tokio::net::{ tcp, TcpSocket };
use tokio::sync::{ broadcast, mpsc, Mutex, oneshot, RwLock, RwLockReadGuard };
use tokio::task::{ JoinHandle, JoinSet };
use tokio::time;
use tokio_util::sync::CancellationToken;

use crate::circular_buffer::{ self, CircularBuffer };
use crate::device_info::DeviceInfo;
use crate::protocol::{ self, X, Y, R, G, B };

const DEFAULT_COMMAND_TIMEOUT: Duration = Duration::from_secs( 1 );
const DEFAULT_POINT_BUFFER_CAPACITY: usize = 10_000;

pub(crate) type Point = ( X, Y, R, G, B );
pub(crate) type PointTx = circular_buffer::Writer<Point>;
type OnResponseMsg = ( protocol::ControlSignal, protocol::Command, State );

// - - - - - - - - - - - - - - - - - -  Command, Control Signal and Error Enums

/// Commands with arguments
#[derive( Clone, Copy, Debug, PartialEq )]
pub(crate) enum Command {
  Begin{ rate: usize },
  Clear,
  Data{ count: usize },
  Estop,
  Ping,
  Prepare,
  _QueueRate{ rate: usize }, // Not implemented yet.
  Stop
}

#[derive( Debug )]
pub enum Error {
  Command( CommandError ),
  Internal( String )
}

impl fmt::Display for Error {
  fn fmt( &self, f: &mut fmt::Formatter<'_> ) -> fmt::Result {
    match self {
      Self::Command( err ) => write!( f, "Command Error: {err}" ),
      Self::Internal( msg ) => write!( f, "Internal Error: {msg}" )
    }
  }
}

#[derive( Debug )]
pub enum CommandError {
  /// The device responded to command with a `nak` and reason.
  Nak( NakReason ),
  /// The command timed-out while waiting for a device response.
  Timeout
}

impl fmt::Display for CommandError {
  fn fmt( &self, f: &mut fmt::Formatter<'_> ) -> fmt::Result {
    match self {
      Self::Nak( reason ) => write!( f, "Nak: {reason}" ),
      Self::Timeout => write!( f, "Timeout" )
    }
  }
}

#[derive( Clone, Copy, Debug, PartialEq )]
pub enum NakReason {
  /// The command was refused as the device is in an E-stop.
  Estop,
  /// The point data command was ignored because the device's point buffer is
  /// full.
  Full,
  /// The command was invalid or malformed.
  Invalid
}

impl fmt::Display for NakReason {
  fn fmt( &self, f: &mut fmt::Formatter<'_> ) -> fmt::Result {
    match self {
      Self::Estop => write!( f, "E-stop" ),
      Self::Full => write!( f, "Full" ),
      Self::Invalid => write!( f, "Invalid" )
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  State

/// The shared client state that includes `last_seen_at`.
#[derive( Clone, Copy, Debug )]
pub struct State {
  inner: protocol::State,
  last_seen_at: Instant
}

impl State {
  /// Returns true if the device is ready to receive point data.
  pub fn is_ready( &self ) -> bool { self.inner.is_ready() }
  /// Returns true if the device is playing point data.
  pub fn is_playing( &self ) -> bool { self.inner.is_playing() }
  /// Returns the current playback state of the device.
  pub fn playback_state( &self ) -> protocol::PlaybackState { self.inner.playback_state }
  /// Returns the number of points currently buffered in the device.
  pub fn points_buffered( &self ) -> usize { self.inner.points_buffered as usize }
  /// Returns true if the device shutter is open.
  pub fn is_shutter_open( &self ) -> bool { self.inner.is_shutter_open() }
  /// Returns true if the device is in an underflow state.
  pub fn is_underflow( &self ) -> bool { self.inner.is_underflow() }
  /// Returns true if the device is in an e-stop state.
  pub fn is_e_stop( &self ) -> bool { self.inner.is_e_stop() }
  /// Returns the current rate in which the device is playing point data.
  pub fn points_per_second( &self ) -> usize { self.inner.points_per_second as usize }
  /// Returns the total number of points played in this client session.
  pub fn points_lifetime( &self ) -> usize { self.inner.points_lifetime as usize }
  /// Returns the current data source for the device.
  pub fn source( &self ) -> protocol::Source { self.inner.source }
  /// Returns the current light engine state of the device.
  pub fn light_engine_state( &self ) -> protocol::LightEngineState { self.inner.light_engine_state }
}

impl Default for State {
  fn default() -> Self {
    Self{ last_seen_at: Instant::now(), inner: protocol::State::default() }
  }
}

/// A read-only version of a `State` shared reference.
pub(crate) struct ReadOnlyState {
  inner: Arc<RwLock<State>>
}

impl ReadOnlyState {
  pub(crate) async fn read( &'_ self ) -> RwLockReadGuard<'_, State> {
    self.inner.read().await
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Client Builder

/// A builder allowing for the configuration and instantiation of a `Client`.
pub struct Builder {
  capacity: usize,
  command_timeout: Duration,
  device_info: DeviceInfo
}

impl Builder {
  pub fn new( device_info: DeviceInfo ) -> Self {
    Self{
      capacity: DEFAULT_POINT_BUFFER_CAPACITY,
      command_timeout: DEFAULT_COMMAND_TIMEOUT,
      device_info
    }
  }

  /// Sets a custom point buffer capacity for the client.
  pub fn capacity( mut self, capacity: usize ) -> Self {
    self.capacity = capacity;
    self
  }

  /// Sets a custom command timeout for the client.
  pub fn command_timeout( mut self, duration: Duration ) -> Self {
    self.command_timeout = duration;
    self
  }

  /// Connects to a remote Etherdream device using the provided configuration.
  pub async fn connect( self ) -> Result<Client,Error> {
    let awaiting_ack = Arc::new( AtomicBool::new( false ) );
    let ( point_tx, point_rx ) = CircularBuffer::new( self.capacity );
    let shutdown_token = CancellationToken::new();
    let state = Arc::new( RwLock::new( State::default() ) );
    let mut tasks = JoinSet::new();

    // Communicates commands from the client to the `<Writer>` task.
    let ( command_tx, command_rx ) = mpsc::channel::<Command>( 16 );

    // Communicates received messages from the `<Reader>` task to all subscribers.
    let ( on_response_tx, _ ) = broadcast::channel::<OnResponseMsg>( 16 );

    // Connect to the Etherdream device
    let dac_stream = TcpSocket::new_v4()?.connect( *self.device_info.address() ).await?;
    let ( dac_rx, dac_tx ) = dac_stream.into_split();

    // Start the `<Reader>` task (w/ cancellation token)
    tasks.spawn({
      let on_response_tx = on_response_tx.clone();
      let shutdown_token = shutdown_token.clone();

      let reader = Reader{
        awaiting_ack: awaiting_ack.clone(),
        dac_rx,
        on_response_tx,
        shutdown_token: shutdown_token.clone(),
        state: state.clone()
      };

      async move {
        tokio::select!{
          _ = shutdown_token.cancelled() => { Ok( () ) }
          result = reader.start() => { result }
        }
      }
    });

    // Start the `<Writer>` task (w/ cancellation token)
    tasks.spawn({
      let shutdown_token = shutdown_token.clone();

      let writer = Writer{
        awaiting_ack: awaiting_ack.clone(),
        command_rx,
        dac_tx,
        device_info: self.device_info.clone(),
        state: state.clone(),
        point_rx,
        shutdown_token: shutdown_token.clone()
      };

      async move {
        tokio::select!{
          _ = shutdown_token.cancelled() => { Ok( () ) }
          result = writer.start() => { result }
        }
      }
    });

    let mut client = Client{
      command_tx: CommandTx::new( command_tx, on_response_tx, self.command_timeout ).await,
      device_info: self.device_info,
      point_tx,
      shutdown_token,
      state,
      tasks
    };

    // Reset the client to a "ready" state
    match client.reset().await {
      Ok( _ ) => Ok( client ),
      Err( err ) => Err( err )
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Client

pub struct Client {
  // The buffer used to communicate command data to the `<Writer>` task.
  command_tx: CommandTx,
  // Intrinsic properties of the remote device.
  device_info: DeviceInfo,
  // The buffer used to communicate point data to the `<Writer>` task.
  point_tx: PointTx,
  // The cancellation token used to shut down the client.
  shutdown_token: CancellationToken,
  // The client's run-time state.
  state: Arc<RwLock<State>>,
  // The asynchronous task handles.
  tasks: JoinSet<io::Result<()>>
}

impl Client {
  /// Returns the socket address of the remote device that the client is
  /// connected to.
  #[inline]
  pub fn peer_addr( &self ) -> &SocketAddr {
    self.device_info.address()
  }

  /// Returns the MAC address of the remote device.
  #[inline]
  pub fn mac_address( &self ) -> &protocol::MacAddress {
    self.device_info.mac_address()
  }

  /// Returns the constant maximum number of points the `Client` can buffer.
  #[inline]
  pub fn max_point_capacity( &self ) -> usize {
    self.point_tx.capacity()
  }

  /// Returns the maximum number of points the device can process per second,
  /// as reported by the device's broadcast payload.
  ///
  /// Important Note:
  /// This number reflects the device's maximum capabilities, which may or may
  /// not match your laser's actual hardware capabilities. The ILDA protocol
  /// does not provide the hardware laser a mechanism to broadcast this
  /// information to the Etherdream DAC.
  ///
  /// It is up to the `Client` implementor to always work within the bounds of
  /// their laser's documented specifications.
  #[inline]
  pub fn max_points_per_second( &self ) -> usize {
    self.device_info.max_points_per_second()
  }

  /// Returns a copy of the connection's current state.
  #[inline]
  pub async fn state( &self ) -> State {
    *self.state.read().await
  }

  /// Ping's the connected Etherdream device and awaits a response.
  #[inline]
  pub async fn ping( &mut self ) -> Result<State,Error> {
    self.command_tx.send_and_wait( Command::Ping ).await
  }

  /// Resets the Etherdream device and returns it to a "prepared" state.
  ///
  /// Specifically:
  ///   1. Will clear any error condition, such as an E-stop or underflow.
  ///   2. Clears the device's internal point buffer.
  ///   3. Resets the device's internal point count to 0.
  pub async fn reset( &mut self ) -> Result<State,Error> {
    self.command_tx.send_and_wait( Command::Clear ).await?;
    self.command_tx.send_and_wait( Command::Prepare ).await
  }

  /// Returns the number of points in the <Client>'s internal buffer.
  #[inline]
  pub fn point_count( &self ) -> usize {
    self.point_tx.len()
  }

  /// Writes point data into client's internal buffer. On success, will return
  /// `None`. If the internal buffer is full, will return `Some::<PointData>`.
  ///
  /// Must call `flush_points` periodically to commit point data to the
  /// asynchronous `<Writer>` task.
  #[inline]
  pub fn push_point( &mut self, x: X, y: Y, r: R, g: G, b: B ) -> Option<Point> {
    self.point_tx.push( ( x, y, r, g, b ) )
  }

  /// Will flush any uncommitted point data to the `<Writer>` task.
  #[inline]
  pub async fn flush_points( &mut self ) -> Result<State,Error> {
    self.command_tx.send_and_wait( Command::Data{ count: self.point_tx.len() } ).await
  }

  /// Commands the device to start playing point data from its internal buffers
  /// at the provided `rate` (points per second).
  pub async fn start( &mut self, rate: usize ) -> Result<State,Error> {
    let point_count = self.point_tx.len();

    if point_count > 0 {
      self.flush_points().await?;
    }

    self.command_tx.send_and_wait( Command::Begin{ rate } ).await
  }

  /// Commands the device to stop playing point data.
  #[inline]
  pub async fn stop( &mut self ) -> Result<State,Error> {
    self.command_tx.send_and_wait( Command::Stop ).await
  }

  /// Commands the device to enter an E-stop.
  #[inline]
  pub async fn e_stop( &mut self ) -> Result<State,Error> {
    self.command_tx.send_and_wait( Command::Estop ).await
  }

  /// Shuts down all asynchronous tasks and disconnects the client from the
  /// device, consuming `self`.
  pub async fn disconnect( self ) {
    self.shutdown_token.cancel();
    self.tasks.join_all().await;
  }

  /// Breaks the client into its constituent parts. Designed for use by the
  /// `Generator`. Crate-internal function, only.
  pub(crate) fn into_parts( self ) -> ( ReadOnlyClient, CommandTx, PointTx ) {
    let client = ReadOnlyClient{
      device_info: self.device_info,
      shutdown_token: self.shutdown_token,
      state: self.state,
      tasks: self.tasks
    };

    ( client, self.command_tx, self.point_tx )
  }

  /// Reconstructs a client from its constituent parts. Designed for use by the
  /// `Generator`. Crate-internal function, only.
  pub(crate) fn from_parts( client: ReadOnlyClient, command_tx: CommandTx, point_tx: PointTx ) -> Client {
    Self{
      command_tx,
      device_info: client.device_info,
      point_tx,
      shutdown_token: client.shutdown_token,
      state: client.state,
      tasks: client.tasks
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Reader

struct Reader {
  awaiting_ack: Arc<AtomicBool>,
  dac_rx: tcp::OwnedReadHalf,
  on_response_tx: broadcast::Sender<OnResponseMsg>,
  shutdown_token: CancellationToken,
  state: Arc<RwLock<State>>
}

impl Reader {
  async fn start( mut self ) -> Result<(),io::Error> {
    let mut buf = [0u8; protocol::RESPONSE_BYTES_SIZE];

    loop {
      if self.shutdown_token.is_cancelled() {
        return Ok( () );
      }

      self.dac_rx.read_exact( &mut buf ).await?;

      let control_signal =
        match buf[0].try_into() {
          Ok( control_signal ) => control_signal,
          Err( unknown ) => {
            // An unknown control signal was received. The reader will shut
            // down as the data stream can no longer be trusted.
            eprintln!( "An unknown control signal was received: {unknown}" );
            self.shutdown_token.cancel();
            continue;
          }
        };

      let command =
        match buf[1].try_into() {
          Ok( command ) => command,
          Err( unknown ) => {
            // An unknown command was received. The reader will shut down as
            // the data stream can no longer be trusted.
            eprintln!( "An unknown command was received: {unknown}" );
            self.shutdown_token.cancel();
            continue;
          }
        };

      {
        let mut state = self.state.write().await;
        state.inner = protocol::State::from_bytes( &buf[2..protocol::RESPONSE_BYTES_SIZE] );
        state.last_seen_at = Instant::now();

        // If a successful acknowledgment, set `awaiting_ack` to false,
        // unblocking the `<Writer>` task.
        self.awaiting_ack.store( false, Release );

        // Publish the response to all subscribers.
        let _ = self.on_response_tx.send( ( control_signal, command, *state ) );
      }
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Writer

struct Writer {
  awaiting_ack: Arc<AtomicBool>,
  command_rx: mpsc::Receiver<Command>,
  dac_tx: tcp::OwnedWriteHalf,
  device_info: DeviceInfo,
  point_rx: circular_buffer::Reader<Point>,
  shutdown_token: CancellationToken,
  state: Arc<RwLock<State>>
}

impl Writer {
  async fn start( mut self ) -> Result<(),io::Error>
  {
    let mut auto_ping_at = Instant::now();
    let mut buf_committed_bytes: usize;
    let mut buf_index: usize;
    let mut buf_point_count: usize;
    let mut point_send_count: usize = 0;
    let mut point_send_count_accumulated: usize = 0;
    let mut state: State;

    // The device can not support more than its intrinsic buffer capacity plus
    // three (3) bytes for the point data header.
    let max_buffer_size: usize = ( self.device_info.buffer_capacity() * protocol::POINT_DATA_BYTES_SIZE ) + 3;
    let mut buf: Vec<u8> = vec![0; max_buffer_size];

    loop {
      if self.shutdown_token.is_cancelled() {
        return Ok( () );
      }

      // Only send commands if all messages have been acknowledged.
      if self.awaiting_ack.load( Acquire ) == true {
        tokio::time::sleep( Duration::from_millis( 1 ) ).await;
        continue;
      }

      // Copy the current device state into our local `state`
      state = *self.state.read().await;

      // Receive any commands and populate the network buffer as required. Any
      // positive `buf_commited_bytes` will be written to the device.
      buf_committed_bytes =
        match self.command_rx.try_recv() {
          Ok( Command::Begin{ rate } ) => {
            buf[0] = protocol::COMMAND_BEGIN;
            buf[1..3].fill( 0 );
            buf[3..7].copy_from_slice( &u32::to_le_bytes( rate as u32 ) );
            7
          },
          Ok( Command::Clear ) => {
            buf[0] = protocol::COMMAND_CLEAR;
            1
          },
          Ok( Command::Data{ count: point_count } ) => {
            point_send_count_accumulated += point_count;
            point_send_count = calculate_point_send_count( &state, point_send_count_accumulated, self.device_info.buffer_capacity() );
            0
          },
          Ok( Command::Estop ) => {
            buf[0] = protocol::COMMAND_ESTOP;
            1
          },
          Ok( Command::Ping ) => {
            buf[0] = protocol::COMMAND_PING;
            1
          },
          Ok( Command::Prepare ) => {
            buf[0] = protocol::COMMAND_PREPARE;
            1
          },
          Ok( Command::_QueueRate{ rate } ) => {
            buf[0] = protocol::COMMAND_QUEUE_RATE;
            buf[1..5].copy_from_slice( &u32::to_le_bytes( rate as u32 ) );
            5
          },
          Ok( Command::Stop ) => {
            buf[0] = protocol::COMMAND_STOP;
            1
          },
          Err( mpsc::error::TryRecvError::Empty ) => {
            // Since no command was received, commit any accumulated points
            point_send_count = calculate_point_send_count( &state, point_send_count_accumulated, self.device_info.buffer_capacity() );

            // Auto-ping: If no point data is available, send a ping every 5ms
            // to ensure our client state reflects the remote device's state.
            if point_send_count == 0 &&
              ( Instant::now() - auto_ping_at ) > Duration::from_millis( 5 )
            {
              buf[0] = protocol::COMMAND_PING;
              auto_ping_at = Instant::now();
              1
            } else {
              0
            }
          },
          Err( mpsc::error::TryRecvError::Disconnected ) => {
            // The command sender has disconnected, the routine should now exit.
            return Ok( () );
          }
        };

      // Attempt to commit any accrued point data to the network buffer. We do
      // this when (1) No other command has been received, and (2) a positive
      // `point_send_count` exists.
      buf_committed_bytes =
        if buf_committed_bytes == 0 && point_send_count > 0 {
          buf_point_count = 0;

          for point_index in 0..point_send_count {
            if let Some( ( x, y, r, g, b ) ) = self.point_rx.pop() {
              buf_index = ( point_index * protocol::POINT_DATA_BYTES_SIZE ) + 3;

              buf[buf_index..buf_index+2].fill( 0 ); // control (unused)
              buf[buf_index+2..buf_index+4].copy_from_slice( &x.to_le_bytes() );
              buf[buf_index+4..buf_index+6].copy_from_slice( &y.to_le_bytes() );
              buf[buf_index+6..buf_index+8].copy_from_slice( &r.to_le_bytes() );
              buf[buf_index+8..buf_index+10].copy_from_slice( &g.to_le_bytes() );
              buf[buf_index+10..buf_index+12].copy_from_slice( &b.to_le_bytes() );
              buf[buf_index+12..buf_index+14].copy_from_slice( &u16::MAX.to_le_bytes() );
              buf[buf_index+14..buf_index+18].fill( 0 );

              buf_point_count += 1;
            } else {
              break;
            }
          }

          // Commit point data (including the 3 byte point header)
          buf[0] = protocol::COMMAND_DATA;
          buf[1..3].copy_from_slice( &u16::to_le_bytes( buf_point_count as u16 ) );
          point_send_count_accumulated = point_send_count_accumulated.saturating_sub( buf_point_count );
          ( buf_point_count * protocol::POINT_DATA_BYTES_SIZE ) + 3
        } else {
          buf_committed_bytes
        };

      // Write any committed bytes to the DAC, or yield control back to the
      // tokio run-time.
      if buf_committed_bytes > 0 {
        self.dac_tx.write_all( &buf[..buf_committed_bytes] ).await?;
        self.awaiting_ack.store( true, Release );
        auto_ping_at = Instant::now();
      } else {
        tokio::task::yield_now().await;
      }
    }
  }
}

/// Calculates the device's available capacity, and returns the minimum of that
/// and the accumulated point count.
#[inline(always)]
fn calculate_point_send_count( state: &State, point_send_count_accumulated: usize, buffer_capacity: usize ) -> usize {
  if state.is_ready() {
    point_send_count_accumulated.min( buffer_capacity.saturating_sub( state.points_buffered() ) )
  } else {
    0
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Read Only Client

/// A read-only version of a `Client` for use by `Generator`s.
pub(crate) struct ReadOnlyClient {
  device_info: DeviceInfo,
  shutdown_token: CancellationToken,
  state: Arc<RwLock<State>>,
  tasks: JoinSet<io::Result<()>>
}

impl ReadOnlyClient {
  /// Returns a read-only clone of the client's state.
  pub(crate) fn clone_state( &self ) -> ReadOnlyState {
    ReadOnlyState{ inner: self.state.clone() }
  }

  /// Returns the `DeviceInfo` associated with this client.
  pub(crate) fn device_info( &self ) -> &DeviceInfo {
    &self.device_info
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Command Tx

type CommandTxWaitForRef = Arc<Mutex<Option<CommandTxWaitFor>>>;

struct CommandTxWaitFor {
  callback: Option<oneshot::Sender<OnResponseMsg>>,
  cmd: protocol::Command,
}

/// Provides functionality for sending commands to the `<Writer>` task and
/// validating acknowledgments.
pub(crate) struct CommandTx {
  // Channel used to send commands to the `<Writer>` task.
  command_tx: mpsc::Sender<Command>,
  // The task handle that consumes all received messages from the `<Reader>`
  _handle: JoinHandle<()>,
  // Used when cloning this `CommandTx`.
  on_response_tx: broadcast::Sender<OnResponseMsg>,
  // The duration of time to wait for command acknowledgements before
  // returning a timeout.
  timeout: Duration,
  // Shared data between `CommandTx` and the asynchronous task.
  wait_for: CommandTxWaitForRef
}

impl CommandTx {
  async fn new(
    command_tx: mpsc::Sender<Command>,
    on_response_tx: broadcast::Sender<OnResponseMsg>,
    timeout: Duration
  ) -> CommandTx {
    let wait_for = Arc::new( Mutex::new( None::<CommandTxWaitFor> ) );

    // Start a task that will receive all responses processed by the `Reader`.
    // If `wait_for` is `Some`, will check each response against
    // `wait_for.command`, executing the callback on a match.
    let handle = tokio::spawn({
      let mut on_response_rx = on_response_tx.subscribe();
      let wait_for = wait_for.clone();

      async move {
        while let Ok( ( control_signal, cmd, state ) ) = on_response_rx.recv().await {
          if let Some( callback ) = wait_for.lock().await
            .take_if( |wf| wf.cmd == cmd )
            .and_then( |mut wf| wf.callback.take() )
          {
            let _ = callback.send( ( control_signal, cmd, state ) );
          }
        }
      }
    });

    Self{
      command_tx,
      _handle: handle,
      on_response_tx,
      timeout,
      wait_for
    }
  }

  /// Will send `command` to the `<Writer>` task and await an acknowledgement.
  pub(crate) async fn send_and_wait( &mut self, command: Command )-> Result<State,Error> {
    let ( wait_for_tx, wait_for_rx ) = oneshot::channel::<OnResponseMsg>();

    // Create our shared "wait_for" payload and send the command
    {
      let mut wait_for = self.wait_for.lock().await;

      *wait_for = Some( CommandTxWaitFor{
        cmd: command.into(),
        callback: Some( wait_for_tx )
      });

      self.command_tx.send( command ).await?;
    }

    // Waits for the command acknowledgement
    match time::timeout( self.timeout, wait_for_rx ).await {
      Ok( Ok( response ) ) => {
        *self.wait_for.lock().await = None; // not needed

        match response {
          ( protocol::ControlSignal::Ack, _, state ) => Ok( state ),
          ( protocol::ControlSignal::NakEstop, _, _ ) => Err( Error::Command( CommandError::Nak( NakReason::Estop ) ) ),
          ( protocol::ControlSignal::NakFull, _, _ ) => Err( Error::Command( CommandError::Nak( NakReason::Full ) ) ),
          ( protocol::ControlSignal::NakInvalid, _, _ ) => Err( Error::Command( CommandError::Nak( NakReason::Invalid ) ) )
        }
      },
      Ok( Err( _ ) ) => {
        // The sender has prematurely closed.
        *self.wait_for.lock().await = None;
        Err( Error::Internal( "The command acknowledgement sender closed.".to_owned() ) )
      },
      Err( _ ) => {
        // The timeout was hit.
        *self.wait_for.lock().await = None;
        Err( Error::Command( CommandError::Timeout ) )
      }
    }
  }

  /// Clones a new `CommandTx`.
  pub(crate) async fn clone( &self ) -> Self {
    Self::new( self.command_tx.clone(), self.on_response_tx.clone(), self.timeout ).await
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Conversions

impl From<io::Error> for Error {
  fn from( err: io::Error ) -> Self {
    Self::Internal( err.to_string() )
  }
}

impl From<mpsc::error::SendError<Command>> for Error {
  fn from( _err: mpsc::error::SendError<Command> ) -> Self {
    Self::Internal( "Command failed to send due to unavailable receiver.".to_owned() )
  }
}

impl From<Command> for protocol::Command {
  fn from( cmd: Command ) -> Self {
    match cmd {
      Command::Begin{ .. } => Self::Begin,
      Command::Clear => Self::Clear,
      Command::Data{ .. } => Self::Data,
      Command::Estop => Self::Estop,
      Command::Ping => Self::Ping,
      Command::Prepare => Self::Prepare,
      Command::_QueueRate{ .. } => Self::QueueRate,
      Command::Stop => Self::Stop
    }
  }
}