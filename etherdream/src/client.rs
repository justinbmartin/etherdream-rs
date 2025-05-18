use core::time;
//use std::future::{ ready, Ready, IntoFuture };
use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::{ atomic, Arc };

use parking_lot::RwLock;
use tokio::io::{ self, AsyncReadExt, AsyncWriteExt };
use tokio::net;
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
  cancel: CancellationToken,
  commands: Arc<RwLock<VecDeque<Command>>>,

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
  }

  // consumes self
  pub async fn stop( self ) {
    self.cancel.cancel();

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
    let cancel = CancellationToken::new();

    let socket = net::TcpSocket::new_v4()?;
    dbg!( self.remote_address );
    let stream = socket.connect( self.remote_address ).await?;
    let ( rx, tx ) = stream.into_split();

    // Read
    let rx_handle = {
      let cancel = cancel.clone();

      tokio::spawn( async move {
        tokio::select!{
          _ = cancel.cancelled() => { Ok(()) }
          result = reader( rx ) => { result }
        }
      })
    };

    // Write
    let tx_handle = {
      let cancel = cancel.clone();
      let commands = commands.clone();

      tokio::spawn( async move {
        tokio::select!{
          _ = cancel.cancelled() => { Ok(()) }
          result = writer( commands, tx ) => { result }
        }
      })
    };

    return Ok( Client{ 
      address: self.remote_address,
      commands: commands,
      cancel: cancel,
      rx_handle: rx_handle,
      tx_handle: tx_handle
    });
  }
}

async fn reader( rx: net::tcp::OwnedReadHalf ) -> io::Result<()> {
  let mut buf = [0 as u8; 2];
  let mut rx = rx;

  loop {
    println!( "> read await..." );
    let _bytes = rx.read( &mut buf ).await.unwrap();
    println!( "> read..." );
          
    if buf == *b"ap" {
      println!( "> ACK!" );
    }
  }
}

async fn writer( commands: Arc<RwLock<VecDeque<Command>>>, tx: net::tcp::OwnedWriteHalf ) -> io::Result<()> {
  let mut tx = tx;

  loop {
    let command = commands.write().pop_front();

    if command.is_some() {
      match command {
        Some( Command::Ping ) => { 
          println!( "> pre-ping..." );
          let _ = tx.write( b"p" ).await?;
          println!( "> pinged..." );
        },
        None => { 
          tokio::time::sleep( time::Duration::from_millis( 1 ) ).await;
        }
      };
    }
  }
}
