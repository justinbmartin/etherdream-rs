//use std::future::{ ready, Ready, IntoFuture };
use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::{ atomic, Arc };

use parking_lot::RwLock;
use tokio::io::{ self, AsyncReadExt, AsyncWriteExt };
use tokio::net;
use tokio::task;

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
  should_stop: Arc<atomic::AtomicBool>,
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
    self.should_stop.store( true, atomic::Ordering::Relaxed );

    let _ = self.rx_handle.await.unwrap();
    let _ = self.tx_handle.await.unwrap();
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
    let should_stop = Arc::new( atomic::AtomicBool::new( true ) );

    let socket = net::TcpSocket::new_v4()?;
    let stream = socket.connect( self.remote_address ).await?;
    let ( rx, mut tx ) = stream.into_split();

    // Read
    let rx_handle = tokio::spawn({
      let should_stop = should_stop.clone();

      async move{
        let mut buf = [0 as u8; 2];

        loop {
          if should_stop.load( atomic::Ordering::Relaxed ) {
            break;
          }

          let _bytes = rx.try_read( &mut buf )?;

          if buf == *b"ap" {
            println!( "> ACK!" );
          }
        }
  
        return io::Result::Ok(());
      }
    });

    // Write
    let tx_handle = tokio::spawn({
      let commands = commands.clone();
      let should_stop = should_stop.clone();

      async move {
        loop {
          if should_stop.load( atomic::Ordering::Relaxed ) {
            break;
          }

          let command = commands.write().pop_front();

          if command.is_some() {
            match command {
              Some( Command::Ping ) => { 
                let _ = tx.try_write( b"p" );
                break;
              },
              None => { }
            };
          }
        }

        return io::Result::Ok(());
      }
    });

    return Ok( Client{ 
      address: self.remote_address,
      commands: commands,
      should_stop: Arc::new( atomic::AtomicBool::new( true ) ),
      rx_handle: rx_handle,
      tx_handle: tx_handle
    });
  }
}
