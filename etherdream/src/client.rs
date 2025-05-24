//use std::mem::transmute;
use std::net::SocketAddr;
use std::ops::Deref;
use std::sync::Arc;

use parking_lot::RwLock;
use tokio::io::{ self, AsyncReadExt, AsyncWriteExt };
use tokio::net;
use tokio::sync::mpsc;
use tokio::task;
use tokio_util::sync::CancellationToken;

//use tokio::net::TcpStream;
//use tokio_util::codec::{ BytesCodec, FramedRead };

use crate::device::State;

pub const DEFAULT_PORT: u16 = 7765;


// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Control Signal

const CONTROL_SIGNAL_ACK: u8 = b'a';
const CONTROL_SIGNAL_NAK: u8 = b'F';
const CONTROL_SIGNAL_INVALID: u8 = b'I';
const CONTROL_SIGNAL_STOP: u8 = b'!';

#[repr( u8 )]
pub enum ControlSignal {
  Ack = CONTROL_SIGNAL_ACK,
  Nak = CONTROL_SIGNAL_NAK,
  Invalid = CONTROL_SIGNAL_INVALID,
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

#[repr( C )]
pub struct EtherdreamResponse {
  // The control signal of the response, can be:
  // 'a':	ACK (0x61) - Acknowledged. The previous command was accepted.
  // 'F': NAK (0x46) - Full. The write command could not be performed because 
  //      there was not enough buffer space when it was received.
  // 'I': NAK (0x49) - Invalid. The command contained an invalid command byte 
  //      or parameters.
  // '!': NAK (0x21) - Stop Condition. An emergency-stop condition still 
  //      exists.
  signal: u8,

  // An echo of the command sent.
  command: u8,
  state: State
}



pub struct Client {
  address: SocketAddr,

  //
  cancellable: CancellationToken,

  //
  command_tx: mpsc::UnboundedSender<Command>,
  
  //
  rx_handle: task::JoinHandle<io::Result<()>>,
  tx_handle: task::JoinHandle<io::Result<()>>,

  //
  current_state: Arc<RwLock<[u8; 20]>>,
  awaiting_command_acks: usize
}

impl Client {
  pub fn remote( &self ) -> SocketAddr {
    return self.address;
  }

  pub fn ping( &mut self ) {
    self.awaiting_command_acks += 1;
    let _ = self.send( Command::Ping );
  }

  fn send( &mut self, command: Command ) -> Result<(),mpsc::error::SendError<Command>> {
    return self.command_tx.send( command );
  }

  // consumes self?
  pub async fn stop( self ) {
    if ! self.cancellable.is_cancelled() {
      self.cancellable.cancel();
    }

    let _ = self.rx_handle.await;
    let _ = self.tx_handle.await;
  }
}

pub struct Builder {
  duration: Option<tokio::time::Duration>,
  remote_address: SocketAddr
}

impl Builder {
  pub fn new( remote_address: SocketAddr ) -> Self {
    return Self{ 
      duration: None,
      remote_address: remote_address
    };
  }

  pub async fn start( &self ) -> io::Result<Client> {
    let cancellable = CancellationToken::new();
    let ( command_tx, command_rx ) = mpsc::unbounded_channel::<Command>();
    let current_state = Arc::new( RwLock::new( [0u8;20] ) );
    
    let socket = net::TcpSocket::new_v4()?;
    let stream = socket.connect( self.remote_address ).await?;
    let ( rx, tx ) = stream.into_split();

    // Spawn read handler
    let rx_handle = {
      let cancellable = cancellable.clone();

      tokio::spawn({
        let current_state = current_state.clone();

        async move {
          tokio::select!{
            _ = cancellable.cancelled() => { Ok(()) }
            result = do_read( current_state, rx ) => { result }
          }
        }
      })
    };

    // Spawn write handler
    let tx_handle = {
      let cancellable = cancellable.clone();

      tokio::spawn( async move {
        tokio::select!{
          _ = cancellable.cancelled() => { Ok(()) }
          result = do_write( command_rx, tx ) => { result }
        }
      })
    };

    if let Some( duration ) = self.duration {
      tokio::spawn({
        let cancellable = cancellable.clone();

        async move {
          tokio::time::sleep( duration ).await;
          cancellable.cancel();
        }
      });
    }

    return Ok( Client{ 
      address: self.remote_address,
      awaiting_command_acks: 0,
      command_tx: command_tx,
      cancellable: cancellable,
      current_state: current_state,
      rx_handle: rx_handle,
      tx_handle: tx_handle
    });
  }
}

async fn do_read( current_state: Arc<RwLock<[u8;20]>>, mut rx: net::tcp::OwnedReadHalf ) -> io::Result<()> {
  let mut buf = [0 as u8; 22]; // TODO: use size of reponse struct/packed

  loop {
    let _ = rx.read_exact( &mut buf ).await?;
    
    // Unpack control data
    let control_signal: ControlSignal = buf[0].into();
    let command: Command = buf[1].into();

    //
    current_state.write().copy_from_slice( &buf[2..22] );

    match control_signal {
      ControlSignal::Ack => {
        println!( "> CLIENT: ACK RECEIVED!" );

        match command {
          Command::Ping => {
            // trait: on_ping( payload ); always
            // channel: ch.send( Ping( payload ) ).await confusing for call-site
            // callback: on_callback( payload ) (can check if set or not)
            println!( "> CLIENT: PING RECEIVED!" );
          }
        }
      },
      _ => {
        println!( "> CLIENT: unknown reply" );
      }
    }
  }
}

async fn do_write( mut command_rx: mpsc::UnboundedReceiver<Command>, mut tx: net::tcp::OwnedWriteHalf ) -> io::Result<()> {
  loop {
    while let Some( command ) = command_rx.recv().await {
      match command {
        Command::Ping => {
          let _ = tx.write( b"p" ).await?;
        }
      }
    }
  }
}
