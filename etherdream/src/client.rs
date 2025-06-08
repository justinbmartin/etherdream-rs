use std::net::SocketAddr;
use std::sync::Arc;

use parking_lot::RwLock;
use tokio::io::{ self, AsyncReadExt, AsyncWriteExt };
use tokio::net::{ self, tcp::{ OwnedReadHalf, OwnedWriteHalf } };
use tokio::sync::{ mpsc, oneshot };
use tokio::task;
use tokio::time;
use tokio_util::sync::CancellationToken;

use crate::device::{self, DEVICE_STATE_BYTES_SIZE};

const DAC_COMMAND_PING: u8              = b'?';
const DAC_CONTROL_ACK: u8               = b'a';
const DAC_CONTROL_NAK: u8               = b'F';
const DAC_CONTROL_INVALID: u8           = b'I';
const DAC_CONTROL_STOP: u8              = b'!';
const DAC_RESPONSE_CONTROL_SIZE: usize  = 2;
const DAC_RESPONSE_SIZE: usize          = DAC_RESPONSE_CONTROL_SIZE + device::DEVICE_STATE_BYTES_SIZE;

type PointsBuffered = u16;

type CallbackPayload    = ( ControlSignal, Command, PointsBuffered );
type CallbackReceiver   = mpsc::Receiver<CallbackPayload>;
type CallbackSender     = mpsc::Sender<CallbackPayload>;
type DeviceStateBytes   = [u8;device::DEVICE_STATE_BYTES_SIZE];
type DeviceStateRef     = Arc<RwLock<DeviceStateBytes>>;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Control Signal

#[repr( u8 )]
#[derive( Debug )]
pub enum ControlSignal {
  // The previous command was accepted.
  Ack = DAC_CONTROL_ACK,

  // The write command could not be performed because there was not
  // enough buffer space when it was received.
  Nak = DAC_CONTROL_NAK,

  // The command contained an invalid command byte or parameters.
  Invalid = DAC_CONTROL_INVALID,

  // An emergency-stop condition exists.
  Stop = DAC_CONTROL_STOP
}

impl ControlSignal {
  pub fn from_byte( signal: u8 ) -> Option<Self> {
    return match signal {
      DAC_CONTROL_ACK      => Some( ControlSignal::Ack ),
      DAC_CONTROL_NAK      => Some( ControlSignal::Nak ),
      DAC_CONTROL_INVALID  => Some( ControlSignal::Invalid ),
      DAC_CONTROL_STOP     => Some( ControlSignal::Stop ),
      _                    => None
    };
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Command

#[repr( u8 )]
#[derive( Debug )]
pub enum Command {
  Ping    = DAC_COMMAND_PING
}

impl Command {
  pub fn from_byte( command: u8 ) -> Option<Self> {
    return match command {
      DAC_COMMAND_PING  => Some( Command::Ping ),
      _                 => None
    };
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Client

pub struct Client {
  // The remote address for the Etherdream DAC
  address: SocketAddr,

  // Token used to shutdown the asynchronous client tasks
  shutdown_token: CancellationToken,

  // Channel used to send commands from the client to the async writer task
  command_tx: mpsc::Sender<Command>,
  
  // The clients async task handles
  tasks: Option<[task::JoinHandle<io::Result<()>>; 3]>,

  // The last recorded state of the Etherdream DAC
  _state: DeviceStateRef,

  // Channel used to acknowledge user-initiated ping requests
  ping_notifier: Arc<RwLock<Option<oneshot::Sender<device::State>>>>
}
 
impl Client {
  pub async fn connect<T>( address: SocketAddr, on_command_handler: T ) -> io::Result<Client>
    where T: Fn( ControlSignal, Command, PointsBuffered ) + Send + 'static
  {
    let ping_notifier = Arc::new( RwLock::new( None::<oneshot::Sender<device::State>> ) );
    let shutdown_token = CancellationToken::new();
    let state = Arc::new( RwLock::new( DeviceStateBytes::default() ) );

    // Responsible for communicating responses received from the DAC to the
    // asynchronous user-provided callback notifier, `do_callback`
    let ( callback_tx, callback_rx ) = mpsc::channel::<CallbackPayload>( 64 );

    // Responsible to communicating commands from the client run-time to the
    // asynchronous DAC writer, `do_write`
    let ( command_tx, command_rx ) = mpsc::channel::<Command>( 64 );

    // Connect to the Etherdream DAC at `address`
    let dac_stream = net::TcpSocket::new_v4()?.connect( address ).await?;
    let ( dac_rx, dac_tx ) = dac_stream.into_split();

    // Start the callback/notification handler
    let callback_handle = {
      let shutdown_token = shutdown_token.child_token();

      tokio::spawn( async move {
        tokio::select!{
          _ = shutdown_token.cancelled() => { Ok(()) }
          result = do_callback( callback_rx, on_command_handler ) => { result }
        }
      })
    };

    // Start the DAC read handler
    let dac_rx_handle = {
      let shutdown_token = shutdown_token.child_token();

      tokio::spawn({
        let ping_notifier = ping_notifier.clone();
        let state = state.clone();

        async move {
          tokio::select!{
            _ = shutdown_token.cancelled() => { Ok(()) }
            result = do_read( state, dac_rx, callback_tx, ping_notifier ) => { result }
          }
        }
      })
    };

    // Start the DAC write handler
    let dac_tx_handle = {
      let shutdown_token = shutdown_token.child_token();

      tokio::spawn( async move {
        tokio::select!{
          _ = shutdown_token.cancelled() => { Ok(()) }
          result = do_write( command_rx, dac_tx ) => { result }
        }
      })
    };

    return Ok( Client{ 
      address: address,
      command_tx: command_tx,
      ping_notifier: ping_notifier,
      shutdown_token: shutdown_token,
      _state: state,
      tasks: Some([ dac_rx_handle, dac_tx_handle, callback_handle ])
    });
  }

  // Stops the client and consumes self. Returns Ok on success.
  pub async fn disconnect( mut self ) -> Result<(),task::JoinError> {
    self.shutdown_token.cancel();

    if let Some( tasks ) = self.tasks.take() {
      for task in tasks {
        let _ = task.await?;
      }
    }

    return Ok(());
  }

  // Return the remote address for the DAC that this client is connected to.
  pub fn remote( &self ) -> SocketAddr {
    return self.address;
  }

  // Send a ping request to the DAC. Response will be delivered asynchronously
  // via the user-provided callback.
  pub async fn ping( &self ) -> Result<device::State,String> {
    //
    let( ping_tx, ping_rx ) = tokio::sync::oneshot::channel::<device::State>();
    *self.ping_notifier.write() = Some( ping_tx );

    //
    let result = time::timeout( time::Duration::from_secs( 2 ), async move {
      let _ = self.command_tx.try_send( Command::Ping );
      
      if let Ok( state ) = ping_rx.await {
        return Ok( state );
      } else {
        return Err( String::from( "failed" ) );
      }
    }).await;

    return match result.unwrap() {
      Ok( result ) => Ok( result ),
      _ => Err( String::from( "CLIENT: Ping timed out..." ) )
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - -  Private async task handlers

async fn do_read( current_state: DeviceStateRef, mut dac_rx: OwnedReadHalf, callback_tx: CallbackSender, ping_notifier: Arc<RwLock<Option<oneshot::Sender<device::State>>>> ) -> io::Result<()> {
  let mut buf = [0u8; DAC_RESPONSE_SIZE];

  loop {
    let _ = dac_rx.read_exact( &mut buf ).await?;
    
    // Unpack control data
    let control_signal = ControlSignal::from_byte( buf[0] );
    let command = Command::from_byte( buf[1] );

    // Copy the state from our buffer into the client
    current_state.write().copy_from_slice( &buf[2..] );

    match control_signal {
      Some( ControlSignal::Ack ) => {
        match command {
          Some( Command::Ping ) => {
            if let Err( _ ) = callback_tx.try_send( ( ControlSignal::Ack, Command::Ping, 0 ) ) {
              // todo: log warning, or call an on_error-like callback?
            }

            if let Some( notifier ) = ping_notifier.write().take() {
              // TODO: this can be more efficient...
              let mut state_bytes = [0; DEVICE_STATE_BYTES_SIZE];
              state_bytes.copy_from_slice( &buf[2..] );

              let _ = notifier.send( device::State::from_bytes( state_bytes ) );
            }
          },
          _ => {
            // todo: log warning? error-like callback?
          }
        }
      },
      _ => {
        todo!();
      }
    }
  }
}

async fn do_write( mut command_rx: mpsc::Receiver<Command>, mut dac_tx: OwnedWriteHalf ) -> io::Result<()> {
  loop {
    while let Some( command ) = command_rx.recv().await {
      match command {
        Command::Ping => {
          let _ = dac_tx.write_u8( DAC_COMMAND_PING ).await?;
        }
      }
    }
  }
}

async fn do_callback<T>( mut callback_rx: CallbackReceiver, on_command_handler: T ) -> io::Result<()> 
  where T: Fn( ControlSignal, Command, PointsBuffered ) + Send + 'static
{
  loop {
    if let Some( ( control_signal, command, points_buffered ) ) = callback_rx.recv().await {
      on_command_handler( control_signal, command, points_buffered );
    }
  }
}