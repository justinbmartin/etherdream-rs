use std::collections::VecDeque;
use std::future::Future;
use std::net::SocketAddr;
use std::ops::DerefMut;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{ Context, Poll };
use std::time;

use parking_lot::RwLock;
use tokio::io::{ self, AsyncReadExt, AsyncWriteExt };
use tokio::net;
use tokio::sync::Notify;
use tokio::task;
use tokio_util::sync::CancellationToken;

//use tokio::net::TcpStream;
//use tokio_util::codec::{ BytesCodec, FramedRead };

use crate::device::State;

pub const DEFAULT_PORT: u16 = 7765;

#[repr( C )]
struct EtherdreamResponse {
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

enum Command {
  Ping
}

pub struct Client {
  address: SocketAddr,
  cancellable: CancellationToken,

  commands: Arc<RwLock<VecDeque<Command>>>,
  commands_available: Arc<Notify>,
  
  rx_handle: task::JoinHandle<io::Result<()>>,
  tx_handle: task::JoinHandle<io::Result<()>>
}

impl Client {
  pub fn remote( &self ) -> SocketAddr {
    return self.address;
  }

  pub fn ping( &mut self ) {
    // push callback?
    self.commands.write().push_back( Command::Ping );
    self.commands_available.notify_one();
  }

  // consumes self
  pub async fn stop( self ) {
    println!( "> stopping..." );
    self.cancellable.cancel();

    let _ = self.rx_handle.await;
    let _ = self.tx_handle.await;
  }
}

pub struct Builder {
  remote_address: SocketAddr,
}

impl Builder {
  pub fn new( remote_address: SocketAddr ) -> Self {
    return Self{ 
      remote_address: remote_address
    };
  }

  pub async fn start( &self ) -> io::Result<Client> {
    let commands = Arc::new( RwLock::new( VecDeque::with_capacity( 128 ) ) );
    let cancellable = CancellationToken::new();
    let notify = Arc::new( Notify::new() );

    let socket = net::TcpSocket::new_v4()?;
    dbg!( self.remote_address );
    let stream = socket.connect( self.remote_address ).await?;
    let ( rx, tx ) = stream.into_split();

    // Read
    let rx_handle = {
      let cancellable = cancellable.clone();

      tokio::spawn( async move {
        tokio::select!{
          _ = cancellable.cancelled() => { Ok(()) }
          result = reader( rx ) => { result }
        }
      })
    };

    // Write
    let tx_handle = {
      let cancellable = cancellable.clone();
      let commands = commands.clone();
      let notify = notify.clone();

      tokio::spawn( async move {
        tokio::select!{
          _ = cancellable.cancelled() => { Ok(()) }
          result = writer( commands, notify, tx ) => { result }
        }
      })
    };

    return Ok( Client{ 
      address: self.remote_address,
      commands: commands,
      commands_available: notify,
      cancellable: cancellable,
      rx_handle: rx_handle,
      tx_handle: tx_handle
    });
  }
}

async fn reader( mut rx: net::tcp::OwnedReadHalf ) -> io::Result<()> {
  let mut buf = [0 as u8; 2];

  loop {
    println!( "> CLIENT: read await..." );
    let _bytes = rx.read_exact( &mut buf ).await?;
    println!( "> CLIENT: read..." );
    
    if buf == *b"ap" {
      println!( "> CLIENT: ACK!" );
    } else {
      println!( "> CLIENT: huh???" );
    }
  }
}

async fn writer( commands: Arc<RwLock<VecDeque<Command>>>, notify: Arc<Notify>, mut tx: net::tcp::OwnedWriteHalf ) -> io::Result<()> {
  loop {
    notify.notified().await;

    while let Some( command ) = commands.write().pop_front() {
      match command {
        Command::Ping => {
          println!( "> CLIENT: pre-ping..." );
          let bytes = tx.write( b"p" ).await?;
          dbg!( bytes );
          println!( "> CLIENT: pinged..." );  
        }
      }
    }
  }
}
