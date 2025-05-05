use std::net::SocketAddr;

use tokio::net::TcpSocket;

use crate::{ Device, device::State };

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

pub struct Client {
  points: Vec<u32>
}

impl Client {
  pub fn new() -> Self {
    return Self{ points: Vec::new() };
  }

  pub fn push_points() {

  }
}

pub struct Builder {
  address: SocketAddr
}

impl Builder {
  pub fn new( address: SocketAddr ) -> Self {
    return Self{ address: address };
  }

  pub async fn run( &self ) -> std::io::Result<()> {
    let socket = TcpSocket::new_v4()?;

    let mut stream = socket.connect( self.address ).await?;
    let ( r, w ) = stream.split();

    tokio::spawn(async move {
      if let Err(e) = listen( r ).await {
        println!("failed to process connection; error = {e}");
      }
    });

    return Ok(());
  }
}

async fn listen( r : tokio::net::tcp::ReadHalf<'_> ) {
  // 1. parse etherdream respose from bytes
  // 2. parse and update device state, persist "somewhere"
  //

  
}