use std::collections::VecDeque;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{ Context, Poll };

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

struct CommandQueue {
  queue: Arc<RwLock<VecDeque<Command>>>,
  notify: Arc<Notify>,
  acks: i64
}

impl CommandQueue {
  fn default() -> Self {
    return Self { 
      queue: Arc::new( RwLock::new( VecDeque::with_capacity( 128 ) ) ),
      notify: Arc::new( Notify::new() ),
      acks: 0
    }
  }

  fn push_back( &mut self, command: Command ) {
    self.queue.write().push_back( command );
    self.notify.notify_one();
  }

  fn pop_front( &mut self ) -> Option<Command> {
    return self.queue.write().pop_front();
  }

  async fn notified( &self ) {
    self.notify.notified().await;
  }
}

pub struct Client {
  address: SocketAddr,

  //
  cancellable: CancellationToken,

  //
  commands: CommandQueue,
  
  //
  rx_handle: task::JoinHandle<io::Result<()>>,
  tx_handle: task::JoinHandle<io::Result<()>>
}

impl Client {
  pub fn remote( &self ) -> SocketAddr {
    return self.address;
  }

  pub fn ping( &mut self ) {
    // push callback?
    self.send( Command::Ping );
  }

  fn send( &mut self, command: Command ) {
    self.commands.push_back( command );
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

  pub fn duration( &mut self, duration: tokio::time::Duration ) -> &mut Self {
    self.duration = Some( duration );
    return self;
  }

  pub async fn start( &self ) -> io::Result<Client> {
    let commands = CommandQueue::default();
    let cancellable = CancellationToken::new();

    let socket = net::TcpSocket::new_v4()?;
    let stream = socket.connect( self.remote_address ).await?;
    let ( rx, tx ) = stream.into_split();

    // Spawn read handler
    let rx_handle = {
      let cancellable = cancellable.clone();

      tokio::spawn( async move {
        tokio::select!{
          _ = cancellable.cancelled() => { Ok(()) }
          result = do_read( rx ) => { result }
        }
      })
    };

    // Spawn write handler
    let tx_handle = {
      let cancellable = cancellable.clone();
      let queue = commands.queue.clone();
      let notify = commands.notify.clone();

      tokio::spawn( async move {
        tokio::select!{
          _ = cancellable.cancelled() => { Ok(()) }
          result = do_write( queue, notify, tx ) => { result }
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
      commands: commands,
      cancellable: cancellable,
      rx_handle: rx_handle,
      tx_handle: tx_handle
    });
  }
}

async fn do_read( mut rx: net::tcp::OwnedReadHalf ) -> io::Result<()> {
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

async fn do_write( queue: Arc<RwLock<VecDeque<Command>>>, notify: Arc<Notify>, mut tx: net::tcp::OwnedWriteHalf ) -> io::Result<()> {
  loop {
    notify.notified().await;

    while let Some( command ) = { queue.write().pop_front() } {
      match command {
        Command::Ping => {
          println!( "> CLIENT: pre-ping..." );
          let _bytes = tx.write( b"p" ).await?;
          println!( "> CLIENT: pinged..." );  
        }
      }
    }
  }
}
