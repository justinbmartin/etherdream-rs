use std::io;
use std::net::{ IpAddr, Ipv4Addr, SocketAddr };
use std::time::Duration;

use tokio::io::{ AsyncReadExt, AsyncWriteExt };
use tokio::net;
use tokio::sync::mpsc;
use tokio::task;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

use etherdream::client;

// Packed version of client::EtherdreamResponse. Read documentation in 
// client::EtherdreamResponse for property descriptions.
#[repr( C, packed )]
#[derive( Default )]
struct EtherdreamResponse {
  // Control messages
  signal: u8,
  command: u8,

  // Device state
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

struct Etherdream {
  cancellation_token: CancellationToken,
  handle: task::JoinHandle<io::Result<()>>
}

impl Etherdream {
  async fn start() -> io::Result<Etherdream> {
    let cancellation_token = CancellationToken::new();

    //
    let listener = net::TcpListener::bind( "127.0.0.1:7765" ).await?;

    //
    let handle = tokio::spawn({
      let cancellation_token = cancellation_token.clone();

      async move {
        tokio::select!{
          _ = cancellation_token.cancelled() => { Ok(()) }

          result = async move {
            let mut buf = [0u8; 128];
      
            println!( "> DAC: accepting..." );
            let ( mut stream, remote ) = listener.accept().await?;
            dbg!( remote );
          
            loop {
              println!( "> DAC: reading..." );
              let _ = stream.read( &mut buf ).await?;
              println!( "> DAC: received..." );
      
              match buf[0].into() {
                client::Command::Ping => {
                  println!( "> DAC: sending ping response..." );
                  let response = EtherdreamResponseBuilder::new( client::ControlSignal::Ack, client::Command::Ping ).to_bytes();
                  let _ = stream.write( &response ).await?;
                }
              }
            }
          } => { result }
      }
    }});
    
    return Ok( Etherdream{
      cancellation_token: cancellation_token,
      handle: handle
    });
  }

  async fn shutdown( self ) {
    self.cancellation_token.cancel();
    let _ = self.handle.await;
  }
}

#[tokio::test]
async fn send_ping_and_receive_notification_via_channel() {
  let address = SocketAddr::new( IpAddr::V4( Ipv4Addr::LOCALHOST ), client::DEFAULT_PORT );

  // Create and start an Etherdream DAC
  let dac = Etherdream::start().await.expect( "Failed to start Etherdream mock..." );

  // Setup a channel to receive the ping notification
  let ( tx, mut rx ) = mpsc::channel::<( client::ControlSignal, client::Command )>( 128 );

  // Create and start a client
  let mut client =
    client::Builder::new( address )
    .on_command( tx )
    .start().await.expect( "Failed to connect..." );

  // Send a ping
  let _ = client.ping().await;

  // Setup future to receive the ping
  let handle = timeout( Duration::from_secs( 2 ), async move {
    return match rx.recv().await.unwrap() {
      ( client::ControlSignal::Ack, client::Command::Ping ) => Ok(()),
      ( control_signal, command ) => Err( format!( "Received incorrect notification: control={:?}, command={:?}", control_signal, command ) )
    };
  });

  // Validate that we received the ping
  assert_eq!( Ok(()), handle.await.unwrap() );

  // Shut it down
  let _ = dac.shutdown().await;
}