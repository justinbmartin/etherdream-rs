use std::net::SocketAddr;
use std::sync::{ Arc, atomic::{ AtomicUsize, Ordering::* } };
use std::time::{ Duration, Instant };

use tokio::io::{ self, AsyncReadExt, AsyncWriteExt };
use tokio::net::{ TcpSocket, tcp::{ OwnedReadHalf, OwnedWriteHalf } };
use tokio::sync::{ mpsc, RwLock, watch };
use tokio::task::JoinSet;
use tokio::time;

use crate::circular_buffer::{ self, CircularBuffer };
use crate::constants::*;
use crate::device::{ Device, MacAddress, State };

pub type CommandResult = Result<State,ConnError>;
pub type PointData = ( i16, i16, u16, u16, u16 );

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
pub enum ConnError {
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

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Conn

pub struct Conn {
  // Channel used to send commands from the client to the async writer task
  command_tx: mpsc::Sender<( Command, usize )>,

  // The device that this client connected to when it was initialized.
  device: Device,

  // Used by `send_command_and_wait` to indicate to <Reader> a command to
  // observe and acknowledge.
  on_wait_command: Arc<RwLock<Option<Command>>>,

  // Used by `send_command_and_wait` to receive a command acknowledgement from
  // the <Reader>.
  on_wait_rx: watch::Receiver<( ControlSignal, State )>,

  // The point buffer writer that the client uses to communicate point data to
  // the client's DAC <Writer> task.
  point_tx: circular_buffer::Writer<PointData>,

  // The shared device state as received from the client <Reader>.
  state: Arc<RwLock<State>>,

  // The clients async task handles.
  tasks: JoinSet<Result<(),ConnError>>
}

impl Conn {
  /// Creates a new client builder from a provided `device` and `capacity`.
  pub(crate) async fn connect( device: Device, capacity: usize ) -> Result<Conn,ConnError> {
    let awaiting_ack_count = Arc::new( AtomicUsize::new( 0 ) );
    let last_seen_at = Arc::new( RwLock::new( Instant::now() ) );
    let on_wait_command = Arc::new( RwLock::new( None ) );
    let ( on_wait_tx, on_wait_rx ) = watch::channel( ( ControlSignal::Ack, State::default() ) );
    let mut tasks: JoinSet<Result<(),ConnError>> = JoinSet::new();
    let state = Arc::new( RwLock::new( device.state().clone() ) );

    // Responsible for communicating commands from the client run-time to the
    // asynchronous DAC network writer (tx)
    let ( command_tx, command_rx ) = mpsc::channel::<( Command, usize )>( 16 );

    // Create the internal point buffer
    let ( point_tx, point_rx ) = CircularBuffer::<PointData>::new( capacity );
    let point_rx = Arc::new( RwLock::new( point_rx ) );

    // Connect to the Etherdream DAC at `address`
    let dac_stream = TcpSocket::new_v4()?.connect( device.address() ).await?;
    let ( dac_rx, dac_tx ) = dac_stream.into_split();

    // Start the network reader
    tasks.spawn({
      let awaiting_ack_count = awaiting_ack_count.clone();
      let last_seen_at = last_seen_at.clone();
      let on_wait_command = on_wait_command.clone();
      let state = state.clone();

      let mut reader = Reader{
        awaiting_ack_count,
        dac_rx,
        last_seen_at,
        on_wait_command,
        on_wait_tx,
        state
      };

      async move { reader.start().await }
    });

    // Start the network writer
    tasks.spawn({
      let awaiting_ack_count = awaiting_ack_count.clone();
      let point_rx = point_rx.clone();
      let state = state.clone();

      let mut writer = Writer{
        awaiting_ack_count,
        command_rx,
        dac_buffer_capacity: device.buffer_capacity() as usize,
        dac_tx,
        point_rx,
        state
      };

      async move { writer.start().await }
    });

    let mut conn = Conn{
      command_tx,
      device,
      on_wait_command,
      on_wait_rx,
      point_tx,
      state,
      tasks
    };

    // Reset the Etherdream DAC which:
    // (1) ensures the DAC is prepared to receive point data, and...
    // (2) ensures the DAC's most current state is reflected in our client.
    match conn.reset().await {
      Ok( _ ) => Ok( conn ),
      Err( err ) => Err( err )
    }
  }

  /// Returns the socket address of the remote DAC that the client is connected
  /// to.
  #[inline]
  pub fn peer_addr( &self ) -> SocketAddr {
    self.device.address()
  }

  /// Returns the MAC address of the remote DAC.
  #[inline]
  pub fn mac_address( &self ) -> MacAddress {
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

  /// Ping's the connected Etherdream DAC and awaits a response. Returns the
  /// <device::State> on success.
  pub async fn ping( &mut self ) -> CommandResult {
    self.send_command_and_wait( Command::Ping, 0 ).await
  }

  /// Returns the Etherdream DAC to a "prepared" state. Specifically:
  ///   1. Will clear any DAC error condition, such as an E-stop or underflow.
  ///   2. Clears the DAC's internal point buffer.
  ///   3. Resets the DAC's internal point count to 0.
  pub(crate) async fn reset( &mut self ) -> CommandResult {
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
  pub(crate) fn push_point( &mut self, x: i16, y: i16, r: u16, g: u16, b: u16 ) -> Option<PointData> {
    self.point_tx.push( ( x, y, r, g, b ) )
  }

  /// Flushes all accumulated point data to the DAC. Returns the number of
  /// points committed. The actual network send happens asynchronously in the
  /// `Writer` task at a future time.
  pub(crate) async fn flush_points( &self ) -> usize {
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
  pub(crate) async fn flush_points_and_wait( &mut self ) -> CommandResult {
    self.send_command_and_wait( Command::Data, self.point_tx.len() ).await
  }

  /// Returns the number of points in the clients internal buffer. These points
  /// are awaiting being sent to the DAC.
  #[inline]
  pub(crate) fn point_count( &self ) -> usize {
    self.point_tx.len()
  }

  /// Sends a message to the DAC to start playing point data at the provided
  /// `rate`.
  ///
  /// Any points currently stored in the client's internal buffer will be
  /// flushed to the DAC prior to sending a `Begin` command.
  pub(crate) async fn start( &mut self, rate: usize ) -> CommandResult {
    let point_count = self.point_tx.len();

    if point_count > 0 {
      self.send_command_and_wait( Command::Data, point_count ).await?;
    }

    self.send_command_and_wait( Command::Begin, rate ).await
  }

  /// Sends a message to the DAC to stop playing point data.
  pub(crate) async fn stop( &mut self ) -> CommandResult {
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

      self.command_tx.send( ( command, n ) ).await.map_err(|_|{ ConnError::Command( CommandError::Shutdown ) })?;
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
          ControlSignal::Nak( reason ) => Err( ConnError::Command( CommandError::Nak( reason ) ) )
        },

      Err( _ ) =>
        Err( ConnError::Command( CommandError::Timeout ) )
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Network Reader

struct Reader {
  awaiting_ack_count: Arc<AtomicUsize>,
  dac_rx: OwnedReadHalf,
  last_seen_at: Arc<RwLock<Instant>>,
  on_wait_command: Arc<RwLock<Option<Command>>>,
  on_wait_tx: watch::Sender<( ControlSignal, State )>,
  state: Arc<RwLock<State>>
}

impl Reader {
  async fn start( &mut self ) -> Result<(),ConnError> {
    let mut buf = [0u8; ETHERDREAM_RESPONSE_BYTES];
    let mut dac_state: State;

    loop {
      self.dac_rx.read_exact( &mut buf ).await?;
      let control_signal = ControlSignal::from( buf[0] );
      let command = Command::from( buf[1] );

      dac_state = State::from_bytes( &buf[2..ETHERDREAM_RESPONSE_BYTES] );
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
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Network Writer

struct Writer {
  awaiting_ack_count: Arc<AtomicUsize>,
  command_rx: mpsc::Receiver<( Command, usize )>,
  dac_buffer_capacity: usize,
  dac_tx: OwnedWriteHalf,
  point_rx: Arc<RwLock<circular_buffer::Reader<PointData>>>,
  state: Arc<RwLock<State>>
}

impl Writer {
  async fn start( &mut self ) -> Result<(),ConnError> {
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
    let mut state: State;

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
            if let Some( ( x, y, r, g, b  ) ) = self.point_rx.write().await.pop() {
              buf_index = ( point_index * ETHERDREAM_POINT_DATA_BYTES ) + 3;

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
        self.dac_tx.write_all( &buf[..buf_committed_bytes] ).await?;
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
fn calculate_point_send_count( state: &State, points_acc_send_count: usize, buffer_capacity: usize ) -> usize {
  if state.is_ready() {
    points_acc_send_count.min( buffer_capacity.saturating_sub( state.points_buffered as usize ) )
  } else {
    0
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

impl From<io::Error> for ConnError {
  fn from( err: io::Error ) -> ConnError {
    ConnError::Io( err )
  }
}