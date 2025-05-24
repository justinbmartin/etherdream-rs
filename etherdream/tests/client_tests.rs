use std::io;
use std::net::{ IpAddr, Ipv4Addr, SocketAddr };

use tokio::io::{ AsyncReadExt, AsyncWriteExt };
use tokio::net;
use tokio::task;

use etherdream::client;

// Packed version of client::EtherdreamResponse. Read documentation in 
// client::EtherdreamResponse for property descriptions.
#[repr( C, packed )]
#[derive( Default )]
struct EtherdreamResponse {
  // Control
  signal: u8,
  command: u8,

  // Device State
  protocol: u8,
  light_engine_state: u8,
  playback_state: u8,
  source: u8,
  light_engine_flags: u16,
  playback_flags: u16,
  source_flags: u16,
  points_buffered: u16,
  points_per_second: u32,
  points_lifetime: u32
}

const ETHERDREAM_RESPONSE_SIZE: usize = size_of::<EtherdreamResponse>();

// Convenience struct and routines to build responses for unit tests.
struct EtherdreamResponseBuilder {
  response: EtherdreamResponse
}

impl EtherdreamResponseBuilder {
  fn new( signal: client::ControlSignal, command: client::Command ) -> Self {
    return Self{
      response: EtherdreamResponse { 
        signal: signal as u8, 
        command: command as u8,
        ..Default::default() 
      }
    };
  }

  fn to_bytes( self ) -> [u8; ETHERDREAM_RESPONSE_SIZE] {
    // SAFETY:
    // * `EtherdreamResponse` has the same size as `[u8; ETHERDREAM_RESPONSE_SIZE]`.
    // * `[u8; ETHERDREAM_RESPONSE_SIZE]` has no alignment requirement.
    // * Since it is packed, this type has no padding.
    unsafe {
      return std::mem::transmute( self.response );
    }
  }
}

async fn start_etherdream( _address: SocketAddr ) -> io::Result<task::JoinHandle<io::Result<()>>> {
  let listener = net::TcpListener::bind( "127.0.0.1:7765" ).await?;
    
  let handle = tokio::spawn( async move {
    let mut buf = [0u8; 128];

    println!( "> DAC: accepting..." );
    let ( mut stream, remote ) = listener.accept().await?;
    dbg!( remote );
    
    loop {
      println!( "> DAC: reading..." );
      let _ = stream.read( &mut buf ).await?;
      println!( "> DAC: received..." );

      if buf[0]  == b"p"[0] {
        println!( "> DAC: sending ping response..." );

        let response = EtherdreamResponseBuilder::new( client::ControlSignal::Ack, client::Command::Ping ).to_bytes();
        let _ = stream.write( &response ).await?;
      }

      break;
    }

    return Ok(());
  });

  return Ok( handle );
}

#[tokio::test]
async fn sends_a_ping_and_receives_a_callback() {
  let address = SocketAddr::new( IpAddr::V4( Ipv4Addr::LOCALHOST ), client::DEFAULT_PORT );

  // Create and start an Etherdream DAC
  let dac = start_etherdream( address ).await.expect( "Failed to start Etherdream mock..." );

  // Create and start a client
  let mut client = 
    client::Builder::new( address )
    .start().await.expect( "Failed to connect..." );

  // Send a ping
  client.ping();

  tokio::time::sleep( tokio::time::Duration::from_secs( 2 ) ).await;

  //let _ = client.stop().await;
  let _ = dac.await;
}