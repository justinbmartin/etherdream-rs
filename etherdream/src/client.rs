//use std::future::{ ready, Ready, IntoFuture };
use std::net::SocketAddr;
use std::sync::Arc;

use parking_lot::RwLock;
use tokio::io::{ self, AsyncReadExt, AsyncWriteExt };
use tokio::net;

//use tokio::net::TcpStream;
//use tokio_util::codec::{ BytesCodec, FramedRead };

use crate::device::State;

pub const DEFAULT_PORT: u16 = 7765;

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
  commands: Arc<RwLock<std::collections::VecDeque<Command>>>,
  read_handle: tokio::task::JoinHandle<io::Result<()>>,
  write_handle: tokio::task::JoinHandle<io::Result<()>>
}

impl Client {
  pub fn ping( &mut self ) {
    // push callback?
    self.commands.write().push_back( Command::Ping );
  }

  async fn start( address: SocketAddr ) -> io::Result<Self> {
    let commands = Arc::new( RwLock::new( std::collections::VecDeque::with_capacity( 128 ) ) );

    let socket = net::TcpSocket::new_v4()?;
    let stream = socket.connect( address ).await?;
    let ( mut rx, mut tx ) = stream.into_split();

    // Read
    let read_handle = tokio::spawn(async move{
      let mut buf = [0 as u8; 2];

      loop {
        let _bytes = rx.read( &mut buf ).await?;

        if buf == *b"ap" {
          println!( "> ACK!" );
          break;
        }
      }
  
      return io::Result::Ok(());
    });

    // Write
    let write_handle = tokio::spawn({
      let commands = commands.clone();

      async move {
        loop {
          let command = commands.write().pop_front();

          if command.is_some() {
            match command {
              Some( Command::Ping ) => { 
                let _ = tx.write( b"p" ).await;
                break;
              },
              None => { }
            };
          }
        }

        return io::Result::Ok(());
      }
    });

    return Ok( Self{ 
      commands: commands,
      read_handle: read_handle,
      write_handle: write_handle
    });
  }

  pub async fn stop( self ) {
    let _ = self.read_handle.await.unwrap();
    let _ = self.write_handle.await.unwrap();
  }
}

pub struct Builder {
  address: SocketAddr,
}

impl Builder {
  pub fn new( address: SocketAddr ) -> Self {
    return Self{ 
      address: address
    };
  }

  pub async fn start( &self ) -> Client {
    return Client::start( self.address ).await.unwrap();
  }
}
