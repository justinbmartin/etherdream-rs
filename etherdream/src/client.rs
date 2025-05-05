use std::net::SocketAddr;

use tokio::{ io::AsyncReadExt, net::TcpSocket };
//use tokio::net::TcpStream;
//use tokio_util::codec::{ BytesCodec, FramedRead };

use crate::device::State;

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
  points: Vec<u32>,
  //stream: TcpStream
}

impl Client {
  pub fn push_points() {

  }
}

pub struct Builder {
  address: SocketAddr,
}

impl Builder {
  pub fn new( address: SocketAddr ) -> Self {
    return Self{ address: address };
  }

  pub async fn start( &self ) -> std::io::Result<()> {
    let socket = TcpSocket::new_v4()?;

    let stream = socket.connect( self.address ).await?;
    //let client = Client{ stream: stream };

    let ( mut read, mut _write ) = stream.into_split();

    // Read
    tokio::spawn( async move {
      let mut buf = [0u8;36];

      while let Ok( _length ) = read.read_exact( &mut buf ).await {
        // parse buf into something
      }
    });

    return Ok( () );
  }
}