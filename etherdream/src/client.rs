//! A client that can communicate with an Etherdream DAC.
use std::net::SocketAddr;
use std::sync::{ Arc, RwLock };
use std::time::Instant;

use tokio::io::{ self, AsyncReadExt, AsyncWriteExt };
use tokio::net::{ self, tcp::{ OwnedReadHalf, OwnedWriteHalf } };
use tokio::sync::{ mpsc, oneshot };
use tokio::task;
use tokio::time;
use tokio_util::sync::CancellationToken;

use crate::circular_buffer;
use crate::device;

const DAC_COMMAND_BEGIN: u8         = b'b';
const DAC_COMMAND_CLEAR: u8         = b'c';
const DAC_COMMAND_DATA: u8          = b'd';
const DAC_COMMAND_PING: u8          = b'?';
const DAC_COMMAND_PREPARE: u8       = b'p';
const DAC_COMMAND_STOP: u8          = b's';
const DAC_CONTROL_ACK: u8           = b'a';
const DAC_CONTROL_NAK_FULL: u8      = b'F';
const DAC_CONTROL_NAK_INVALID: u8   = b'I';
const DAC_CONTROL_NAK_STOP: u8      = b'!';

const DEFAULT_CLIENT_POINT_CAPACITY: usize = 30_000;

type InstantRef = Arc<RwLock<Instant>>;
type PointBufferType = ( i16, i16, u16, u16, u16 );
type StateRef = Arc<RwLock<device::State>>;
type WaitingCommandCallbackRef = Arc<RwLock<Option<( Command, oneshot::Sender<device::State> )>>>;

// Used internally to send commands from the Client thread to the asynchronous DAC tx thread.
#[derive( Clone, Copy )]
enum Command {
  Begin{ rate: u32 },
  Clear,
  Ping,
  Prepare,
  Stop
}

#[derive( Debug )]
pub enum CommandError {
  Failed,
  Timeout
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Client

/// A client that can communicate with an Etherdream DAC.
pub struct Client<const N: usize = DEFAULT_CLIENT_POINT_CAPACITY> {
  // The remote address of the Etherdream DAC.
  address: SocketAddr,

  // Channel used to send commands from the client to the async writer task
  command_tx: mpsc::Sender<Command>,

  // The time that the client was created
  created_at: Instant,

  // The time that the client last received a message from the DAC
  last_seen_at: InstantRef,

  // The buffer writer to send points to
  point_tx: circular_buffer::Writer<PointBufferType,N>,

  // Token used to shut down asynchronous client tasks
  shutdown_token: CancellationToken,

  // The last recorded state of the Etherdream DAC
  state: StateRef,

  // The clients async task handles
  tasks: Option<[task::JoinHandle<io::Result<()>>; 2]>,

  // The sender of a one-shot channel used for `send_command_and_wait` calls
  waiting_command_callback: WaitingCommandCallbackRef,
}
 
impl<const N: usize> Client<N> {
  /// Creates a new client using a SocketAddr.
  pub async fn new( address: SocketAddr ) -> io::Result<Client<N>>
  {
    let last_seen_at = Arc::new( RwLock::new( Instant::now() ) );
    let ( point_tx, point_rx ) = circular_buffer::CircularBuffer::<PointBufferType,N>::new();
    let shutdown_token = CancellationToken::new();
    let state = Arc::new( RwLock::new( device::State::default() ) );
    let waiting_command_callback = Arc::new( RwLock::new( None ) );

    // Responsible to communicating commands from the client run-time to the
    // asynchronous DAC writer, `do_write`
    let ( command_tx, command_rx ) = mpsc::channel::<Command>( 64 );

    // Connect to the Etherdream DAC at `address`
    let dac_stream = net::TcpSocket::new_v4()?.connect( address ).await?;
    let ( dac_rx, dac_tx ) = dac_stream.into_split();

    // Start the DAC read handler
    let dac_rx_handle =
      tokio::spawn({
        let command_callback = waiting_command_callback.clone();
        let last_seen_at = last_seen_at.clone();
        let shutdown_token = shutdown_token.child_token();
        let state = state.clone();

        async move {
          tokio::select!{
            _ = shutdown_token.cancelled() => { Ok(()) }
            result = do_read( dac_rx, state, last_seen_at, command_callback ) => { result }
          }
        }
      });

    // Start the DAC write handler
    let dac_tx_handle =
      tokio::spawn({ 
        let shutdown_token = shutdown_token.child_token();

        async move {
          tokio::select!{
            _ = shutdown_token.cancelled() => { Ok(()) }
            result = do_write::<N>( dac_tx, command_rx, point_rx ) => { result }
          }
        }
      });
    
    let mut client = Client{ 
      address,
      command_tx,
      created_at: Instant::now(),
      last_seen_at,
      point_tx,
      shutdown_token,
      state,
      tasks: Some([ dac_rx_handle, dac_tx_handle ]),
      waiting_command_callback
    };

    // Reset the Etherdream DAC which:
    // (1) ensures the DAC is prepared to receive point data, and...
    // (2) ensures the DAC's most current state is reflected in our client.
    match client.reset().await {
      Ok( _ ) => Ok( client ),
      Err( _error ) => Err( io::Error::other( "" ) )
    }
  }

  /// Returns the socket address that the Etherdream DAC is connected to.
  pub fn remote( &self ) -> &SocketAddr {
    &self.address
  }

  /// Returns the most recently recorded Etherdream DAC state. For the most up-to-date information,
  /// use `ping`.
  pub fn state( &self ) -> device::State {
    *self.state.read().unwrap()
  }

  /// Returns the duration of time that the client has been active for.
  pub fn elapsed_time( &self ) -> time::Duration {
    *self.last_seen_at.read().unwrap() - self.created_at
  }

  /// Sends a ping to the Etherdream DAC and awaits a response.
  pub async fn ping( &mut self ) -> Result<device::State,CommandError> {
    self.send_command_and_wait( Command::Ping ).await
  }

  /// Resets the Etherdream DAC to a "prepared" state that is ready to receive point data. Will
  /// clear any E-stop flags, if set.
  pub async fn reset( &mut self ) -> Result<device::State,CommandError> {
    match self.send_command_and_wait( Command::Clear ).await {
      Ok( _ ) => self.send_command_and_wait( Command::Prepare ).await,
      error => error
    }
  }

  /// Pushes a single point into the clients internal point buffer. Point data will be
  /// asynchronously streamed to the Etherdream DAC.
  ///
  /// TODO: Allow for queue rate change.
  pub fn push_point( &mut self, x: i16, y: i16, r: u16, g: u16, b: u16 ) {
    let _ = self.point_tx.push(( x, y, r, g, b ));
  }

  /// Sends a message to the DAC to start playing point data at the provided `rate`.
  pub fn start( &self, rate: u32 ) {
    let _ = self.command_tx.try_send( Command::Begin{ rate } );
  }

  /// Sends a message to the DAC to stop playing point data.
  pub async fn stop( &mut self ) -> Result<device::State,CommandError> {
    self.send_command_and_wait( Command::Stop ).await
  }

  /// Stops the client and consumes `self`.
  pub async fn disconnect( mut self ) -> Result<(),task::JoinError> {
    self.shutdown_token.cancel();

    if let Some( tasks ) = self.tasks.take() {
      for task in tasks {
        let _ = task.await?;
      }
    }

    Ok(())
  }

  // Private routine to send a command to the Etherdream DAC and wait for an ACK, or timeout.
  async fn send_command_and_wait( &mut self, command: Command ) -> Result<device::State,CommandError> {
    // Set up a oneshot channel. The sender is stored in our client and will be consumed by the
    // DAC rx thread when a response is received that matches `command`.
    let ( waiting_command_tx, waiting_command_rx ) = oneshot::channel::<device::State>();
    *self.waiting_command_callback.write().unwrap() = Some( ( command, waiting_command_tx ) );

    // Send the command
    let _ = self.command_tx.try_send( command );

    // Wait for the acknowledgement or timeout
    let result = time::timeout( time::Duration::from_secs( 2 ), waiting_command_rx ).await;

    match result {
      Ok( Ok( result ) ) =>
        Ok( result ),
      Ok( Err( _ ) ) =>
        Err( CommandError::Failed ),
      Err( _ ) => {
        // The shared oneshot channel sender should be reset since this is a timeout
        *self.waiting_command_callback.write().unwrap() = None;
        Err( CommandError::Timeout )
      }
    }
  }
}

// ==================================================================== Private

// The DAC will echo back every request with a response that uses this byte layout:
//
// | byte(s)  | description       |
//  ------------------------------
// | 0        | Control Signal    |
// | 1        | Command           |
// | 2-22     | DAC State         |
//
// Definitions:
// Control Signal     One of Ack ('a'), Nak Full ('F'), Nak Invalid ('I') or Nak Stop ('!').
// Command            The command that this response is an echo for. Responses are always sent in
//                    the same order as the requests.
// DAC State          The current device state. Read the documentation of `device::State` for the
//                    byte layout.
async fn do_read(
  mut dac_rx: OwnedReadHalf,
  current_state: StateRef,
  last_seen_at: InstantRef,
  waiting_command_callback: WaitingCommandCallbackRef
) -> io::Result<()>
{
  let mut buf = [0u8; 22];
  let mut state: device::State;
  let mut state_bytes = [0; device::DEVICE_STATE_BYTES_SIZE];

  loop {
    let _ = dac_rx.read_exact( &mut buf ).await?;

    //
    *last_seen_at.write().unwrap() = Instant::now();

    //
    state_bytes.copy_from_slice( &buf[2..] );
    state = device::State::from_bytes( state_bytes );
    *current_state.write().unwrap() = state;

    // Unpack the control signal in byte 0
    match buf[0] {
      DAC_CONTROL_ACK => {
        // Unpack the command in byte 1
        match buf[1] {
          unknown_command => {
            println!( "ACK CMD = {}", unknown_command as char );
            if let Some( ( _cmd, callback2 ) ) = waiting_command_callback.write().unwrap().take() {
              let _ = callback2.send( state );
            }
          }
        }
      },
      DAC_CONTROL_NAK_FULL => {
        println!( "[NAK FULL]: {}", buf[1] );
      },
      DAC_CONTROL_NAK_INVALID => {
        println!( "[NAK INVALID]" );
      },
      DAC_CONTROL_NAK_STOP => {
        println!( "[NAK E-STOP]" );
      },
      _ => {
        println!( "[an unknown control signal was received]" )
      }
    }
  }
}

async fn do_write<const N: usize>(
  mut dac_tx: OwnedWriteHalf,
  mut command_rx: mpsc::Receiver<Command>,
  mut point_rx: circular_buffer::Reader<PointBufferType,N>,
) -> io::Result<()> {
  let mut data = vec!( 0u8; N * 20 ); // TODO: right-size this packed data buffer.
  let mut index: usize = 0;

  // Responsible to sending all command and point data to the Etherdream DAC.
  'primary_loop: loop {

    // Will break out of loop once all commands have been sent.
    'command_loop: loop {

      if ! point_rx.is_empty() {
        //let _count = estimate_dac_buffer_count( state, last_seen_at );
        let point_count = point_rx.len() as u16;

        data[0] = DAC_COMMAND_DATA;
        data[1..3].copy_from_slice( &u16::to_le_bytes( point_count ) ); // number of points

        for point_index in 0..point_count {
          if let Some(( x, y, r, g, b )) = point_rx.pop() {
            index = ( point_index as usize * 18 ) + 3;

            data[index..index+2].fill( 0 ); // control (unused)
            data[index+2..index+4].copy_from_slice( &i16::to_le_bytes( x ) );
            data[index+4..index+6].copy_from_slice( &i16::to_le_bytes( y ) );
            data[index+6..index+8].copy_from_slice( &u16::to_le_bytes( r ) );
            data[index+8..index+10].copy_from_slice( &u16::to_le_bytes( g ) );
            data[index+10..index+12].copy_from_slice( &u16::to_le_bytes( b ) );
            data[index+12..index+14].copy_from_slice( &u16::to_le_bytes( u16::MAX ) );
            data[index+14..index+18].fill( 0 );

            index += 18;
          } else {
            break;
          }
        }

        let _ = dac_tx.write( &data[..index] ).await?;
      }

      //
      match command_rx.try_recv() {
        Ok( Command::Ping ) => {
          let _ = dac_tx.write_u8( DAC_COMMAND_PING ).await?;
        },
        Ok( Command::Prepare ) => {
          let _ = dac_tx.write_u8( DAC_COMMAND_PREPARE ).await?;
        },
        Ok( Command::Begin{ rate } ) => {          
          data[0] = DAC_COMMAND_BEGIN;
          data[1..3].fill( 0 );
          data[3..7].copy_from_slice( &u32::to_le_bytes( rate ) );

          let _ = dac_tx.write( &data[..7] ).await?;
        },
        Ok( Command::Clear ) => {
          let _ = dac_tx.write_u8( DAC_COMMAND_CLEAR ).await?;
        },
        Ok( Command::Stop ) => {
          let _ = dac_tx.write_u8( DAC_COMMAND_STOP ).await?;
        },
        Err( mpsc::error::TryRecvError::Empty ) => {
          // All commands have been sent, break out of the command loop
          break 'command_loop;
        },
        Err( mpsc::error::TryRecvError::Disconnected ) => {
          // The command sender has hung up, gracefully exit this routine
          break 'primary_loop;
        }
      }
    }

    // ...
    tokio::time::sleep( time::Duration::from_micros( 100 ) ).await;
  }

  Ok( () )
}

/*
// Returns an u16 based on the scaled `amount` in range [0.0, 1.0]
fn scaled_u16( amount: f32 ) -> u16 {
  ( u16::MAX as f32 * amount ) as u16
}

// Returns an i16 based on the scaled `amount` in range [0.0, 1.0]
fn scaled_i16( amount: f32 ) -> i16 {
  ( ( u16::MAX as f32 * amount ) - i16::MAX as f32 ) as i16
}
*/

/*
// Returns the estimated number of points in the DAC's internal buffer.
fn _estimate_dac_buffer_count( state: &State, last_seen_at: &Instant ) -> usize {
  // cpp implementation
  //size_t buffer_count = static_cast<size_t>( dac->get_status().points_buffered );
  //
  //if( is_dac_playing( dac ) ) {
  //  auto elapsed_ms = duration_cast<milliseconds>( steady_clock::now() - dac->get_last_seen_at() ).count();
  //  auto estimated_points_consumed = elapsed_ms * ( dac->get_status().points_per_second / 1000 );
  //
  //  buffer_count = std::max( buffer_count - estimated_points_consumed, static_cast<size_t>( 0 ) );
  //}
  //
  //return buffer_count;

  let mut buffer_count = state.points_buffered;

  if state.playback_state == PlaybackState::Playing {
    let elapsed = Instant::now() - *last_seen_at;
    let est_points_consumed = elapsed.as_millis() * ( state.points_buffered / 1000 ) as u128;

    buffer_count = ( buffer_count - ( est_points_consumed as u16 ) ).max( 0 );
  }

  return buffer_count as usize;
}
*/