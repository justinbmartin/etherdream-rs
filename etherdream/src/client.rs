//use std::future::{ ready, Ready, IntoFuture };
use std::net::SocketAddr;
use std::sync::Arc;

use parking_lot::RwLock;
use tokio::io::{ self, AsyncWriteExt };
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
}

impl Client {
  pub fn ping( &mut self ) {
    // push callback?
    self.commands.write().push_back( Command::Ping );
  }

  fn start( address: SocketAddr ) -> Self {
    let commands = Arc::new( RwLock::new( std::collections::VecDeque::with_capacity( 128 ) ) );

    let _write_handle = tokio::spawn({
      let commands = commands.clone();

      async move {
        let socket = net::TcpSocket::new_v4()?;
        let mut stream = socket.connect( address ).await?;
        let ( _rx, mut tx ) = stream.split();

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

          tokio::time::sleep( tokio::time::Duration::from_millis( 1 ) ).await;
        }

        return io::Result::Ok(());
      }
    });

    return Self{ 
      commands: commands,
    }
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
    return Client::start( self.address );
  }
}
