//! An Etherdream DAC network client.
use std::marker::PhantomData;
use std::net::{ IpAddr, SocketAddr };
use std::sync::{ Arc, atomic::{ AtomicUsize, Ordering::* } };
use std::time::{ Duration, Instant };

use tokio::io::{ self, AsyncReadExt, AsyncWriteExt };
use tokio::net::{ TcpSocket, tcp::{ OwnedReadHalf, OwnedWriteHalf } };
use tokio::sync::{ mpsc, RwLock, watch };
use tokio::task::JoinSet;
use tokio::time;

use crate::circular_buffer;
use crate::constants::*;
use crate::device;

type CommandResult = Result<device::State,CommandError>;
type OnLowWatermarkCallback = Box<dyn FnMut( usize ) + Send>;
type OnResponseCallback = fn( ControlSignal, Command, device::State );

const DEFAULT_CLIENT_POINT_CAPACITY: usize = 32_768;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Traits

/// Required trait for <Client> template argument to provide for Etherdream-
/// compliant point data.
///
/// Returns values:
/// ( x_pos, y_pos, red_value, green_value, blue_value )
pub trait Point: 'static + Clone + Copy + Default + Send + Sync {
  fn for_etherdream( &self ) -> ( i16, i16, u16, u16, u16 );
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Enums

/// Commands recognized by an Etherdream DAC.
#[derive( Clone, Copy, Debug, PartialEq )]
pub enum Command {
  Begin,
  Clear,
  Data,
  Estop,
  Ping,
  Prepare,
  QueueRate,
  Stop
}

#[derive( Clone, Copy, Debug, PartialEq )]
pub enum ControlSignal {
  Ack,
  Nak( NakReason )
}

#[derive( Clone, Copy, Debug, PartialEq )]
pub enum NakReason {
  /// The DAC command was refused as the DAC is in an E-stop.
  Estop,

  /// The DAC point data command was ignored because the point buffer is full.
  Full,

  /// The DAC command was malformed.
  Invalid
}

#[derive( Debug )]
pub enum ClientError {
  Command( CommandError ),
  Io( io::Error )
}

#[derive( Debug )]
pub enum CommandError {
  /// The DAC responded with a `nak` and reason.
  Nak( NakReason ),

  /// The <Client> is shutdown. No further commands should be attempted.
  Shutdown,

  /// The command timed-out when waiting for a DAC response.
  Timeout
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Client Builder

/// Client builder to configure and connect to an Etherdream DAC.
pub struct ClientBuilder<T> where T: Point,
{
  capacity: usize,
  device: device::Device,
  low_watermark: usize,
  on_low_watermark: Option<OnLowWatermarkCallback>,
  on_response: Option<OnResponseCallback>,
  phantom: PhantomData<T>
}

impl<T> ClientBuilder<T> where T: Point
{
  /// Creates a new client builder from a provided `device`.
  pub fn new( device: device::Device ) -> Self {
    Self{
      capacity: DEFAULT_CLIENT_POINT_CAPACITY,
      device,
      low_watermark: 0,
      on_low_watermark: None,
      on_response: None,
      phantom: PhantomData
    }
  }

  /// Creates a new client builder from provided device properties.
  pub fn from_properties( ip: IpAddr, intrinsics: device::Intrinsics ) -> Self {
    let socket_addr = SocketAddr::new( ip, ETHERDREAM_CLIENT_PORT );
    let device = device::Device::from_parts( socket_addr, intrinsics, device::State::default() );
    Self::new( device )
  }

  /// Defines the internal client's buffer capacity. Defaults to
  /// `DEFAULT_CLIENT_POINT_CAPACITY`.
  pub fn capacity( mut self, capacity: usize ) -> Self {
    self.capacity = capacity;
    self
  }

  /// Provides the caller an ability to receive a callback on each DAC
  /// response. Callback should not block for any significant duration of time.
  pub fn on_response( mut self, callback: OnResponseCallback ) -> Self {
    self.on_response = Some( callback );
    self
  }

  /// Provides the caller an ability to define a callback that the <Client>
  /// will execute once the DAC's point count drops below `count`. The provided
  /// `callback` should not significantly block.
  ///
  /// The `callback` will only execute a single-time once the low watermark
  /// range is entered. The low watermark warning will be reset once the
  /// client's point count reports greater than `count`.
  ///
  /// The timing resolution of this routine is ~1ms.
  pub fn on_low_watermark<F>( mut self, count: usize, callback: F ) -> Self
    where F: 'static + FnMut( usize ) + Send
  {
    self.low_watermark = count;
    self.on_low_watermark = Some( Box::new( callback ) );
    self
  }

  /// Creates a new `Client` that connects to the remote DAC. Consumes `self`.
  pub async fn connect( self ) -> Result<Client<T>,ClientError> {
    let awaiting_ack_count = Arc::new( AtomicUsize::new( 0 ) );
    let last_seen_at = Arc::new( RwLock::new( Instant::now() ) );
    let on_wait_command = Arc::new( RwLock::new( None ) );
    let ( on_wait_tx, on_wait_rx ) = watch::channel( ( ControlSignal::Ack, device::State::default() ) );
    let mut task_set: JoinSet<Result<(),ClientError>> = JoinSet::new();
    let state = Arc::new( RwLock::new( self.device.state().clone() ) );

    // Responsible for communicating commands from the client run-time to the
    // asynchronous DAC network writer (tx)
    let ( command_tx, command_rx ) = mpsc::channel::<( Command, usize )>( 16 );

    // Create the internal point buffer
    let ( point_tx, point_rx ) = circular_buffer::CircularBuffer::<T>::new( self.capacity );
    let point_rx = Arc::new( RwLock::new( point_rx ) );

    // Connect to the Etherdream DAC at `address`
    let dac_stream = TcpSocket::new_v4()?.connect( self.device.address() ).await?;
    let ( dac_rx, dac_tx ) = dac_stream.into_split();

    // Start the DAC network reader
    task_set.spawn({
      let awaiting_ack_count = awaiting_ack_count.clone();
      let last_seen_at = last_seen_at.clone();
      let on_wait_command = on_wait_command.clone();
      let state = state.clone();

      let mut reader = Reader{
        awaiting_ack_count,
        dac_rx,
        last_seen_at,
        on_response: self.on_response,
        on_wait_command,
        on_wait_tx,
        state
      };

      async move { reader.start().await }
    });

    // Start the DAC network writer
    task_set.spawn({
      let awaiting_ack_count = awaiting_ack_count.clone();
      let point_rx = point_rx.clone();
      let state = state.clone();

      let mut writer = Writer::<T>{
        awaiting_ack_count,
        command_rx,
        dac_buffer_capacity: self.device.buffer_capacity() as usize,
        dac_tx,
        point_rx,
        state
      };

      async move { writer.start().await }
    });

    // Start the (optional) low watermark service
    if let Some( callback ) = self.on_low_watermark {
      task_set.spawn({
        let point_rx = point_rx.clone();
        let state = state.clone();

        let mut low_watermark_service =
          LowWatermarkService{
            callback,
            low_watermark: self.low_watermark,
            point_rx,
            state
          };

        async move { low_watermark_service.start().await }
      });
    }

    let mut client = Client{
      command_tx,
      device: self.device,
      on_wait_command,
      on_wait_rx,
      point_tx,
      state,
      tasks: task_set
    };

    // Reset the Etherdream DAC which:
    // (1) ensures the DAC is prepared to receive point data, and...
    // (2) ensures the DAC's most current state is reflected in our client.
    match client.reset().await {
      Ok( _ ) => Ok( client ),
      Err( error ) => Err( ClientError::Command( error ) )
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Client

/// A client that can communicate with an Etherdream DAC.
pub struct Client<T: Point> {
  // Channel used to send commands from the client to the async writer task
  command_tx: mpsc::Sender<( Command, usize )>,

  // The device that this client connected to when it was initialized.
  device: device::Device,

  // The point buffer writer that the client uses to communicate point data to
  // the client's DAC <Writer> task.
  point_tx: circular_buffer::Writer<T>,

  // Used by `send_command_and_wait` to indicate to <Reader> a command to
  // observe and acknowledge.
  on_wait_command: Arc<RwLock<Option<Command>>>,

  // Used by `send_command_and_wait` to receive a command acknowledgement from
  // the <Reader>.
  on_wait_rx: watch::Receiver<( ControlSignal, device::State )>,

  // The shared device state as received from the client <Reader>.
  state: Arc<RwLock<device::State>>,

  // The clients async task handles.
  tasks: JoinSet<Result<(),ClientError>>,
}

impl<T: Point> Client<T> {
  /// Returns the socket address of the remote DAC that the client is connected
  /// to.
  #[inline]
  pub fn peer_addr( &self ) -> SocketAddr {
    self.device.address()
  }

  /// Returns the MAC address of the remote DAC.
  #[inline]
  pub fn mac_address( &self ) -> device::MacAddress {
    self.device.mac_address()
  }

  /// Returns the maximum number of points the DAC can process per second, as
  /// reported by the device's broadcast payload.
  ///
  /// Important Note:
  /// This number reflects the DAC's maximum capabilities, which may or
  /// not match your laser's actual hardware capabilities. The ILDA protocol
  /// does not provide the hardware laser a mechanism to broadcast this
  /// information to the DAC.
  ///
  /// It is up to the `Client` implementor to always work within the bounds of
  /// their laser's documented specifications.
  #[inline]
  pub fn max_points_per_second( &self ) -> usize {
    self.device.max_points_per_second() as usize
  }

  /// Copies the most recently recorded DAC state into the user-provided
  /// `state`.
  #[inline]
  pub async fn copy_state( &self, state: &mut device::State ) {
    *state = *self.state.read().await;
  }

  /// Ping's the connected Etherdream DAC and awaits a response. Returns the
  /// <device::State> on success.
  pub async fn ping( &mut self ) -> CommandResult {
    self.send_command_and_wait( Command::Ping, 0 ).await
  }

  /// Returns the Etherdream DAC to a "prepared" state. Specifically:
  ///   1. Will clear any DAC error condition, such as an E-stop or underflow.
  ///   2. Clears the DAC's internal point buffer.
  ///   3. Resets the DAC's internal point count to 0.
  pub async fn reset( &mut self ) -> CommandResult {
    match self.send_command_and_wait( Command::Clear, 0 ).await {
      Ok( _ ) => self.send_command_and_wait( Command::Prepare, 0 ).await,
      err => err
    }
  }

  /// Pushes a single point of type <T> into the client's internal point
  /// buffer.
  ///
  /// Point data is published to the DAC:
  ///   1. When `flush_points` is called.
  ///   2. When `start` is called and no existing point data has been
  ///      published.
  ///   3. When the <Writer> observes unpublished point data and the DAC is in
  ///      a playing state.
  ///
  /// TODO: Allow for queue rate change.
  #[inline]
  pub fn push_point( &mut self, point: T ) -> Option<T> {
    self.point_tx.push( point )
  }

  /// Flushes all accumulated point data to the DAC. Returns the number of
  /// points committed. The actual network send happens asynchronously in the
  /// `Writer` task at a future time.
  pub async fn flush_points( &self ) -> usize {
    let count = self.point_tx.len();
    if count == 0 { return 0; }

    if let Ok(()) = self.command_tx.send( ( Command::Data, count ) ).await {
      count
    } else {
      0
    }
  }

  /// Flushes all accumulated point data to the DAC and awaits an
  /// acknowledgement. Returns the device state on success.
  pub async fn flush_points_and_wait( &mut self ) -> CommandResult {
    self.send_command_and_wait( Command::Data, self.point_tx.len() ).await
  }

  /// Returns the number of points in the clients internal buffer. These points
  /// are awaiting being sent to the DAC.
  #[inline]
  pub fn point_count( &self ) -> usize {
    self.point_tx.len()
  }

  /// Sends a message to the DAC to start playing point data at the provided
  /// `rate`.
  ///
  /// Any points currently stored in the client's internal buffer will be
  /// flushed to the DAC prior to sending a `Begin` command.
  pub async fn start( &mut self, rate: usize ) -> CommandResult {
    let point_count = self.point_tx.len();

    if point_count > 0 {
      self.send_command_and_wait( Command::Data, point_count ).await?;
    }

    self.send_command_and_wait( Command::Begin, rate ).await
  }

  /// Sends a message to the DAC to stop playing point data.
  pub async fn stop( &mut self ) -> CommandResult {
    self.send_command_and_wait( Command::Stop, 0 ).await
  }

  /// Terminates all asynchronous tasks and consumes `self`.
  pub async fn disconnect( mut self ) {
    self.tasks.abort_all();
  }

  async fn send_command_and_wait( &mut self, command: Command, n: usize )-> CommandResult {
    {
      let mut guard = self.on_wait_command.write().await;
      *guard = Some( command );

      self.on_wait_rx.mark_unchanged();

      self.command_tx.send( ( command, n ) ).await.map_err(|_|{ CommandError::Shutdown })?;
    }

    // Wait for a command acknowledgement with `on_wait_rx` or timeout.
    let result =
      time::timeout(
        Duration::from_secs( 2 ),
        async {
          let _ = self.on_wait_rx.changed().await;
          *self.on_wait_rx.borrow_and_update()
        }
      ).await;

    // Reset `on_wait_command`
    *self.on_wait_command.write().await = None;

    match result {
      Ok( ( signal, state ) ) =>
        match signal {
          ControlSignal::Ack => Ok( state ),
          ControlSignal::Nak( reason ) => Err( CommandError::Nak( reason ) )
        },

      Err( _ ) =>
        Err( CommandError::Timeout )
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - DAC Reader

struct Reader {
  awaiting_ack_count: Arc<AtomicUsize>,
  dac_rx: OwnedReadHalf,
  last_seen_at: Arc<RwLock<Instant>>,
  on_response: Option<OnResponseCallback>,
  on_wait_command: Arc<RwLock<Option<Command>>>,
  on_wait_tx: watch::Sender<( ControlSignal, device::State )>,
  state: Arc<RwLock<device::State>>
}

impl Reader {
  async fn start( &mut self ) -> Result<(),ClientError> {
    let mut buf = [0u8; ETHERDREAM_RESPONSE_BYTES];
    let mut dac_state: device::State;

    loop {
      self.dac_rx.read_exact( &mut buf ).await?;
      let control_signal = ControlSignal::from( buf[0] );
      let command = Command::from( buf[1] );

      dac_state = device::State::from_bytes( &buf[2..ETHERDREAM_RESPONSE_BYTES] );
      *self.state.write().await = dac_state;

      // Update the client's `last_seen_at` to the current moment
      *self.last_seen_at.write().await = Instant::now();

      // Decrement the awaiting ack count (this unblocks the <Writer>)
      if control_signal == ControlSignal::Ack {
        let _ = self.awaiting_ack_count.fetch_update( Release, Acquire, |i|{ Some( i.saturating_sub( 1 ) ) });
      }

      // If an `on_wait_command` exists, validate it against the currently
      // received command and acknowledge the message.
      {
        let mut guard = self.on_wait_command.write().await;
        if let Some( _ ) = guard.take_if(|on_wait_cmd|{ *on_wait_cmd == command }) {
          let _ = self.on_wait_tx.send( ( control_signal, dac_state ) );
        }
      }

      // Execute the `on_response` callback, if it is defined.
      if let Some( callback ) = self.on_response {
        callback( control_signal, command, *self.state.read().await );
      }
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - DAC Writer

struct Writer<T: Point> {
  awaiting_ack_count: Arc<AtomicUsize>,
  command_rx: mpsc::Receiver<( Command, usize )>,
  dac_buffer_capacity: usize,
  dac_tx: OwnedWriteHalf,
  point_rx: Arc<RwLock<circular_buffer::Reader<T>>>,
  state: Arc<RwLock<device::State>>
}

impl<T: Point> Writer<T> {
  async fn start( &mut self ) -> Result<(),ClientError> {
    // The DAC can not support more than its intrinsic buffer capacity plus
    // three (3) bytes for the point data header.
    let max_buffer_size: usize = ( self.dac_buffer_capacity * ETHERDREAM_POINT_DATA_BYTES ) + 3;

    // Locals
    let mut auto_ping_at = Instant::now();
    let mut buf: Vec<u8> = vec![0; max_buffer_size];
    let mut buf_committed_bytes: usize;
    let mut buf_index: usize;
    let mut buf_point_count: usize;
    let mut point_send_count: usize = 0;
    let mut points_acc_send_count: usize = 0;
    let mut state: device::State;

    loop {
      // Only send DAC commands if all messages have been acknowledged.
      if self.awaiting_ack_count.load( Acquire ) > 0 {
        tokio::time::sleep( Duration::from_millis( 1 ) ).await;
        continue;
      }

      // Copy the current DAC state into our local `state`
      state = *self.state.read().await;

      // Receive any commands and populate the network buffer as required. Any
      // positive `buf_commited_bytes` will be written to the DAC.
      buf_committed_bytes =
        match self.command_rx.try_recv() {
          Ok( ( Command::Begin, rate ) ) => {
            buf[0] = ETHERDREAM_COMMAND_BEGIN;
            buf[1..3].fill( 0 );
            buf[3..7].copy_from_slice( &u32::to_le_bytes( rate as u32 ) );
            7
          },

          Ok( ( Command::Data, point_count ) ) => {
            // Because the DAC has a limited buffer in comparison to our
            // client, we accumulate `point_count`. This allows our client to
            // continue publishing point data to DAC as buffer capacity becomes
            // available.
            points_acc_send_count += point_count;
            point_send_count = calculate_point_send_count( &state, points_acc_send_count, self.dac_buffer_capacity );
            0
          },

          // Sends any other single-byte command to the DAC
          Ok( ( cmd, _ ) ) => {
            buf[0] = cmd.into();
            1
          },

          Err( mpsc::error::TryRecvError::Empty ) => {
            // Since no command was received, commit any accumulated points
            point_send_count = calculate_point_send_count( &state, points_acc_send_count, self.dac_buffer_capacity );

            // Auto-ping: If no point data is available, send a ping every 5ms
            // to ensure our client state reflects the remote DAC's state.
            if point_send_count == 0 &&
              ( Instant::now() - auto_ping_at ) > Duration::from_millis( 5 )
            {
              buf[0] = Command::Ping.into();
              auto_ping_at = Instant::now();
              1
            } else {
              0
            }
          },

          Err( mpsc::error::TryRecvError::Disconnected ) => {
            // The command sender has disconnected, the routine should now exit.
            return Ok(());
          }
        };

      // Attempt to commit any accrued point data to the network buffer. We do
      // this when (1) No other command has been received, and (2) a positive
      // `point_send_count` exists.
      buf_committed_bytes =
        if buf_committed_bytes == 0 && point_send_count > 0 {
          buf_point_count = 0;

          for point_index in 0..point_send_count {
            if let Some( point ) = self.point_rx.write().await.pop() {
              buf_index = ( point_index * ETHERDREAM_POINT_DATA_BYTES ) + 3;
              let( x, y, r, g, b ) = point.for_etherdream();

              buf[buf_index..buf_index+2].fill( 0 ); // control (unused)
              buf[buf_index+2..buf_index+4].copy_from_slice( &i16::to_le_bytes( x ) );
              buf[buf_index+4..buf_index+6].copy_from_slice( &i16::to_le_bytes( y ) );
              buf[buf_index+6..buf_index+8].copy_from_slice( &u16::to_le_bytes( r ) );
              buf[buf_index+8..buf_index+10].copy_from_slice( &u16::to_le_bytes( g ) );
              buf[buf_index+10..buf_index+12].copy_from_slice( &u16::to_le_bytes( b ) );
              buf[buf_index+12..buf_index+14].copy_from_slice( &u16::to_le_bytes( u16::MAX ) );
              buf[buf_index+14..buf_index+18].fill( 0 );

              buf_point_count += 1;
            } else {
              break;
            }
          }

          // Commit point data (including the 3 byte point header)
          buf[0] = ETHERDREAM_COMMAND_DATA;
          buf[1..3].copy_from_slice( &u16::to_le_bytes( buf_point_count as u16 ) );
          points_acc_send_count = points_acc_send_count.saturating_sub( buf_point_count );
          ( buf_point_count * ETHERDREAM_POINT_DATA_BYTES ) + 3
        } else {
          buf_committed_bytes
        };

      // Write any committed bytes to the DAC, or yield control back to the
      // tokio run-time.
      if buf_committed_bytes > 0 {
        self.dac_tx.write( &buf[..buf_committed_bytes] ).await?;
        self.awaiting_ack_count.fetch_add( 1, AcqRel );
      } else {
        tokio::task::yield_now().await;
      }
    }
  }
}

/// Calculates the DAC's available capacity, and returns the minimum of that
/// and the accumulated point count.
#[inline]
fn calculate_point_send_count( state: &device::State, points_acc_send_count: usize, buffer_capacity: usize ) -> usize {
  if state.is_ready() {
    points_acc_send_count.min( buffer_capacity.saturating_sub( state.points_buffered as usize ) )
  } else {
    0
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - -  Low Watermark Service

struct LowWatermarkService<T: Point> {
  callback: OnLowWatermarkCallback,
  low_watermark: usize,
  point_rx: Arc<RwLock<circular_buffer::Reader<T>>>,
  state: Arc<RwLock<device::State>>
}

impl<T: Point> LowWatermarkService<T> {
  async fn start( &mut self ) -> Result<(),ClientError> {
    let mut in_low_watermark = false;
    let mut state: device::State;

    loop {
      state = *self.state.read().await;

      if state.is_playing() {
        // If the active point count drops below the `low_watermark`, execute
        // the user-provided `callback`. The callback will not be executed
        // again until the point count has recovered to above the
        // `low_watermark`.
        let active_point_count = state.points_buffered as usize + self.point_rx.read().await.len();

        if in_low_watermark {
          if active_point_count > self.low_watermark {
            in_low_watermark = false;
          }
        } else {
          if active_point_count <= self.low_watermark {
            in_low_watermark = true;
            ( self.callback )( active_point_count );
          }
        }
      }

      // Yield control back to the tokio control time via `sleep`
      tokio::time::sleep( Duration::from_millis( 1 ) ).await;
    }
  }
}

//  - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

impl From<u8> for ControlSignal {
  fn from( byte: u8 ) -> ControlSignal {
    match byte {
      ETHERDREAM_CONTROL_ACK => ControlSignal::Ack,
      ETHERDREAM_CONTROL_NAK_FULL => ControlSignal::Nak( NakReason::Full ),
      ETHERDREAM_CONTROL_NAK_ESTOP => ControlSignal::Nak( NakReason::Estop ),
      ETHERDREAM_CONTROL_NAK_INVALID => ControlSignal::Nak( NakReason::Invalid ),
      byte => {
        // An unknown control signal was received. This will be translated to
        // an E-stop and logged.
        eprintln!( "An unknown control signal was received: {byte}" );
        ControlSignal::Nak( NakReason::Estop )
      }
    }
  }
}

impl From<Command> for u8 {
  fn from( cmd: Command ) -> u8 {
    match cmd {
      Command::Begin => ETHERDREAM_COMMAND_BEGIN,
      Command::Clear => ETHERDREAM_COMMAND_CLEAR,
      Command::Data => ETHERDREAM_COMMAND_DATA,
      Command::Estop => ETHERDREAM_COMMAND_ESTOP,
      Command::Ping => ETHERDREAM_COMMAND_PING,
      Command::Prepare => ETHERDREAM_COMMAND_PREPARE,
      Command::QueueRate => ETHERDREAM_COMMAND_QUEUE_RATE,
      Command::Stop => ETHERDREAM_COMMAND_STOP,
    }
  }
}

impl From<u8> for Command {
  fn from( byte: u8 ) -> Command {
    match byte {
      ETHERDREAM_COMMAND_BEGIN => Command::Begin,
      ETHERDREAM_COMMAND_CLEAR => Command::Clear,
      ETHERDREAM_COMMAND_DATA => Command::Data,
      ETHERDREAM_COMMAND_ESTOP => Command::Estop,
      ETHERDREAM_COMMAND_PING => Command::Ping,
      ETHERDREAM_COMMAND_PREPARE => Command::Prepare,
      ETHERDREAM_COMMAND_QUEUE_RATE => Command::QueueRate,
      ETHERDREAM_COMMAND_STOP => Command::Stop,
      byte => {
        // An unknown command was received. This will be translated to an
        // E-stop and logged.
        eprintln!( "An unknown command was received: {byte}" );
        Command::Estop
      }
    }
  }
}

impl From<io::Error> for ClientError {
  fn from( err: io::Error ) -> ClientError {
    ClientError::Io( err )
  }
}