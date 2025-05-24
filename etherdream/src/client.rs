use std::net::SocketAddr;
use std::sync::Arc;

use parking_lot::RwLock;
use tokio::io::{ self, AsyncReadExt, AsyncWriteExt };
use tokio::net::{ self, tcp };
use tokio::sync::mpsc::{ self, error::SendError };
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
  // Acknowledged. The previous command was accepted.
  Ack = CONTROL_SIGNAL_ACK,

  // Full. The write command could not be performed because there was not
  // enough buffer space when it was received.
  Nak = CONTROL_SIGNAL_NAK,

  // Invalid. The command contained an invalid command byte or parameters.
  Invalid = CONTROL_SIGNAL_INVALID,

  // Stop Condition. An emergency-stop condition still exists.
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
  address: SocketAddr,

  // Token used to shutdown the client
  shutdown_token: CancellationToken,

  // Channel used to send commands from the client to the async writer
  command_tx: mpsc::Sender<Command>,
  
  //
  tasks: Option<[task::JoinHandle<io::Result<()>>; 2]>,

  //
  current_state: Arc<RwLock<[u8; 20]>>,
  awaiting_command_acks: usize
}

impl Client {
  pub async fn start( address: SocketAddr ) -> io::Result<Client> {
    return do_start( address, None ).await;
  }

  pub async fn start_with_notifications( address: SocketAddr, tx_channel: mpsc::Sender<( ControlSignal, Command )> ) -> io::Result<Client> {
    return do_start( address, Some( tx_channel ) ).await;
  }

  // Returns the remote address this client is connected to.
  pub fn remote( &self ) -> SocketAddr {
    return self.address;
  }

  // Sends a ping request to the DAC. If 
  pub async fn ping( &mut self ) -> Result<(),SendError<Command>> {
    self.awaiting_command_acks += 1;
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

async fn do_start( address: SocketAddr, tx_channel: Option<mpsc::Sender<( ControlSignal, Command )>> ) -> io::Result<Client> {
  let shutdown_token = CancellationToken::new();
  let ( command_tx, command_rx ) = mpsc::channel::<Command>( 128 );
  let current_state = Arc::new( RwLock::new( [0u8;20] ) );
  
  let socket = net::TcpSocket::new_v4()?;
  let stream = socket.connect( address ).await?;
  let ( rx, tx ) = stream.into_split();

  // Spawn read handler
  let rx_handle = {
    let shutdown_token = shutdown_token.child_token();

    tokio::spawn({
      let current_state = current_state.clone();

      async move {
        tokio::select!{
          _ = shutdown_token.cancelled() => { Ok(()) }
          result = do_read( tx_channel, current_state, rx ) => { result }
        }
      }
    })
  };

  // Spawn write handler
  let tx_handle = {
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
    awaiting_command_acks: 0,
    command_tx: command_tx,
    shutdown_token: shutdown_token,
    current_state: current_state,
    tasks: Some([ rx_handle, tx_handle ])
  });
}

async fn do_read( ch: Option<mpsc::Sender<( ControlSignal, Command )>>, current_state: Arc<RwLock<[u8;20]>>, mut rx: tcp::OwnedReadHalf ) -> io::Result<()> {
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
            if let Some( ch ) = &ch {
              let _ = ch.send( ( control_signal, command ) ).await;
            }
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
