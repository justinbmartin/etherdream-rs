use std::net::SocketAddr;
use std::sync::{ Arc, atomic::{ AtomicUsize, Ordering } };

use parking_lot::RwLock;
use tokio::io::{ self, AsyncReadExt, AsyncWriteExt };
use tokio::net::{ self, tcp };
use tokio::{ sync, sync::mpsc::{ self, error::SendError } };
use tokio::task;
use tokio_util::sync::CancellationToken;

pub const DEFAULT_PORT: u16 = 7765;


// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Control Signal

const CONTROL_SIGNAL_ACK: u8 = b'a';
const CONTROL_SIGNAL_NAK: u8 = b'F';
const CONTROL_SIGNAL_INVALID: u8 = b'I';
const CONTROL_SIGNAL_STOP: u8 = b'!';

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
  state: Arc<RwLock<[u8; 20]>>,

  // A counter of ack's we are expecting to receive from the Etherdream DAC
  awaiting_command_acks: Arc<AtomicUsize>
}

impl Client {
  pub async fn start<T>( address: SocketAddr, on_command_handler: T ) -> io::Result<Client> 
    where T: Fn( ControlSignal, Command ) + Send + 'static
  {
    let notify = Arc::new( sync::Notify::new() );
    let shutdown_token = CancellationToken::new();
    let ( command_tx, command_rx ) = mpsc::channel::<Command>( 128 );
    let state = Arc::new( RwLock::new( [0u8;20] ) );
    let awaiting_command_acks = Arc::new( AtomicUsize::new( 0 ) );
  
    let socket = net::TcpSocket::new_v4()?;
    let stream = socket.connect( address ).await?;
    let ( rx, tx ) = stream.into_split();

    // Spawn notifier handler
    let notifier = {
      let state = state.clone();
      let notify = notify.clone();
      let shutdown_token = shutdown_token.child_token();

      tokio::spawn( async move {
        tokio::select!{
          _ = shutdown_token.cancelled() => { Ok(()) }
          result = do_notify( state, notify, on_command_handler ) => { result }
        }
      })
    };

    // Spawn read handler
    let rx_manager = {
      let awaiting_command_acks = awaiting_command_acks.clone();
      let shutdown_token = shutdown_token.child_token();

      tokio::spawn({
        let state = state.clone();

        async move {
          tokio::select!{
            _ = shutdown_token.cancelled() => { Ok(()) }
            result = do_read( state, notify, rx, awaiting_command_acks ) => { result }
          }
        }
      })
    };

    // Spawn write handler
    let tx_manager = {
      let shutdown_token = shutdown_token.child_token();

      tokio::spawn( async move {
        tokio::select!{
          _ = shutdown_token.cancelled() => { Ok(()) }
          result = do_write( command_rx, tx ) => { result }
        }
      })
    };

    return Ok( Client{ 
      address: address,
      awaiting_command_acks: awaiting_command_acks,
      command_tx: command_tx,
      shutdown_token: shutdown_token,
      state: state,
      tasks: Some([ rx_manager, tx_manager, notifier ])
    });
  }

  // Returns the remote address this client is connected to.
  pub fn remote( &self ) -> SocketAddr {
    return self.address;
  }

  // Sends a ping request to the DAC. If 
  pub async fn ping( &mut self ) -> Result<(),SendError<Command>> {
    self.awaiting_command_acks.fetch_add( 1, Ordering::Relaxed );
    return self.send( Command::Ping ).await;
  }

  async fn send( &mut self, command: Command ) -> Result<(),SendError<Command>> {
    return self.command_tx.send( command ).await;
  }

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

async fn do_read( current_state: Arc<RwLock<[u8;20]>>, notify: Arc<sync::Notify>, mut rx: tcp::OwnedReadHalf, awaiting_command_acks: Arc<AtomicUsize> ) -> io::Result<()> {
  let mut buf = [0 as u8; 22]; // TODO: use size of reponse struct/packed

  loop {
    let _ = rx.read_exact( &mut buf ).await?;
    
    // Unpack control data
    let control_signal: ControlSignal = buf[0].into();
    let command: Command = buf[1].into();

    // Copy the state from our buffer into the client
    current_state.write().copy_from_slice( &buf[2..22] );

    match control_signal {
      ControlSignal::Ack => {
        match command {
          Command::Ping => {
            awaiting_command_acks.fetch_sub( 1, Ordering::Relaxed );
            notify.notify_one();
          }
        }
      },
      _ => {
        todo!()
      }
    }
  }
}

async fn do_write( mut command_rx: mpsc::Receiver<Command>, mut tx: tcp::OwnedWriteHalf ) -> io::Result<()> {
  loop {
    while let Some( command ) = command_rx.recv().await {
      match command {
        Command::Ping => {
          let _ = tx.write_u8( COMMAND_PING ).await?;
        }
      }
    }
  }
}

async fn do_notify<T>( _current_state: Arc<RwLock<[u8;20]>>, notify: Arc<sync::Notify>, on_command_handler: T ) -> io::Result<()> 
  where T: Fn( ControlSignal, Command ) + Send + 'static
{
  loop {
    notify.notified().await;

    //
    on_command_handler( ControlSignal::Ack, Command::Ping );
  }
}