use std::net::SocketAddr;
use std::sync::Arc;

use parking_lot::RwLock;
use tokio::io::{ self, AsyncReadExt, AsyncWriteExt };
use tokio::net::{ self, tcp };
use tokio::sync::mpsc::{ self, error::TrySendError };
use tokio::task;
use tokio_util::sync::CancellationToken;

pub const DEFAULT_PORT: u16 = 7765;

const ETHERDREAM_RESPONSE_CONTROL_SIZE: usize = 2;
const ETHERDREAM_RESPONSE_STATE_SIZE: usize = 20;
const ETHERDREAM_RESPONSE_SIZE: usize = ETHERDREAM_RESPONSE_CONTROL_SIZE + ETHERDREAM_RESPONSE_STATE_SIZE;

type PointsBuffered = u16;

type CallbackPayload = ( ControlSignal, Command, PointsBuffered );
type CallbackReceiver = mpsc::Receiver<CallbackPayload>;
type CallbackSender = mpsc::Sender<CallbackPayload>;
type StateRef = Arc<RwLock<[u8;ETHERDREAM_RESPONSE_STATE_SIZE]>>;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Control Signal

const CONTROL_SIGNAL_ACK: u8      = b'a';
const CONTROL_SIGNAL_NAK: u8      = b'F';
const CONTROL_SIGNAL_INVALID: u8  = b'I';
const CONTROL_SIGNAL_STOP: u8     = b'!';

#[repr( u8 )]
#[derive( Debug )]
pub enum ControlSignal {
  // The previous command was accepted.
  Ack = CONTROL_SIGNAL_ACK,

  // The write command could not be performed because there was not
  // enough buffer space when it was received.
  Nak = CONTROL_SIGNAL_NAK,

  // The command contained an invalid command byte or parameters.
  Invalid = CONTROL_SIGNAL_INVALID,

  // An emergency-stop condition exists.
  Stop = CONTROL_SIGNAL_STOP
}

impl From<u8> for ControlSignal {
  fn from( signal: u8 ) -> Self {
    return match signal {
      CONTROL_SIGNAL_ACK => ControlSignal::Ack,
      CONTROL_SIGNAL_NAK => ControlSignal::Nak,
      CONTROL_SIGNAL_INVALID => ControlSignal::Invalid,
      CONTROL_SIGNAL_STOP => ControlSignal::Stop,
      _ => panic!( "An unknown control signal was provided: {}", signal )
    };
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Command

const COMMAND_PING: u8 = b'p';

#[repr( u8 )]
#[derive( Debug )]
pub enum Command {
  Ping = COMMAND_PING
}

impl From<u8> for Command {
  fn from( command: u8 ) -> Self {
    return match command {
      COMMAND_PING => Command::Ping,
      _ => panic!( "An unknown command was provided: {}", command )
    };
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Client

pub struct Client {
  // The remote address for the Etherdream DAC
  address: SocketAddr,

  // Token used to shutdown the asynchronous client tasks
  shutdown_token: CancellationToken,

  // Channel used to send commands from the run-time client to the async writer task
  command_tx: mpsc::Sender<Command>,
  
  // List of task handles that can be joined against
  tasks: Option<[task::JoinHandle<io::Result<()>>; 3]>,

  // The last recorded state of the Etherdream DAC
  state: Arc<RwLock<[u8; ETHERDREAM_RESPONSE_STATE_SIZE]>>
}
 
impl Client {
  pub async fn start<T>( address: SocketAddr, on_command_handler: T ) -> io::Result<Client> 
    where T: Fn( ControlSignal, Command, PointsBuffered ) + Send + 'static
  {
    let shutdown_token = CancellationToken::new();
    let state = Arc::new( RwLock::new( [0u8; ETHERDREAM_RESPONSE_STATE_SIZE] ) );

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
        let state = state.clone();

        async move {
          tokio::select!{
            _ = shutdown_token.cancelled() => { Ok(()) }
            result = do_read( state, callback_tx, dac_rx ) => { result }
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
      shutdown_token: shutdown_token,
      state: state,
      tasks: Some([ dac_rx_handle, dac_tx_handle, callback_handle ])
    });
  }

  // Return the remote address for the DAC that this client is connected to.
  pub fn remote( &self ) -> SocketAddr {
    return self.address;
  }

  // Send a ping request to the DAC. Response will be delivered asynchronously
  // via the user-provided callback.
  pub fn ping( &mut self ) -> Result<(),TrySendError<Command>> {
    return self.command_tx.try_send( Command::Ping );
  }

  // Stops the client, disconnecting it from the DAC.
  pub async fn stop( &mut self ) -> Result<(),task::JoinError> {
    self.shutdown_token.cancel();

    if let Some( tasks ) = self.tasks.take() {
      for task in tasks {
        let _ = task.await?;
      }
    }

    return Ok(());
  }
}

async fn do_read( current_state: StateRef, callback_tx: CallbackSender, mut dac_rx: tcp::OwnedReadHalf ) -> io::Result<()> {
  let mut buf = [0u8; ETHERDREAM_RESPONSE_SIZE];

  loop {
    let _ = dac_rx.read_exact( &mut buf ).await?;
    
    // Unpack control data
    let control_signal: ControlSignal = buf[0].into();
    let command: Command = buf[1].into();

    // Copy the state from our buffer into the client
    current_state.write().copy_from_slice( &buf[2..] );

    match control_signal {
      ControlSignal::Ack => {
        match command {
          Command::Ping => {
            if let Err( _ ) = callback_tx.try_send( ( control_signal, command, 0 ) ) {
              // todo: log warning, or call an on_error-like callback?
            }
          }
        }
      },
      _ => {
        todo!();
      }
    }
  }
}

async fn do_write( mut client_rx: mpsc::Receiver<Command>, mut dac_tx: tcp::OwnedWriteHalf ) -> io::Result<()> {
  loop {
    while let Some( command ) = client_rx.recv().await {
      match command {
        Command::Ping => {
          let _ = dac_tx.write_u8( COMMAND_PING ).await?;
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