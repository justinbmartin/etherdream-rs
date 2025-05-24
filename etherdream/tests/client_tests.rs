use std::net::{ IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4 };
use std::time::Duration;

use tokio::io::{ AsyncReadExt, AsyncWriteExt };
use tokio::net;
use tokio::sync::mpsc;
use tokio::task;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

use etherdream::client;

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
  shutdown_token: CancellationToken,
  handle: task::JoinHandle<Result<(),&'static str>>
}

impl Etherdream {
  async fn start() -> Etherdream {
    let shutdown_token = CancellationToken::new();

    //
    let address = SocketAddrV4::new( Ipv4Addr::LOCALHOST, client::DEFAULT_PORT );
    let listener = net::TcpListener::bind( address ).await.expect( "Failed to bind mock Etherdream device to address." );

    //
    let handle = tokio::spawn({
      let shutdown_token = shutdown_token.child_token();

      async move {
        tokio::select!{
          _ = shutdown_token.cancelled() => { 
            return Err( "Etherdream task timed out due to test configuration." );
          }

          result = async move {
            let mut buf = [0u8; 128];
      
            let ( mut stream, remote ) = listener.accept().await.expect( "Failed to accept request from listener." );
            dbg!( remote );
          
            loop {
              let _ = stream.read( &mut buf ).await.expect( "Failed to read data from client." );
      
              match buf[0].into() {
                client::Command::Ping => {
                  let response = EtherdreamResponseBuilder::new( client::ControlSignal::Ack, client::Command::Ping ).to_bytes();
                  let _ = stream.write( &response ).await.expect( "Failed to write data to client." );
                }
              }
            }
          } => { result }
      }
    }});
    
    return Etherdream{
      shutdown_token: shutdown_token,
      handle: handle
    };
  }

  async fn shutdown( self ) {
    self.shutdown_token.cancel();
    let _ = self.handle.await;
  }
}

#[tokio::test]
async fn send_ping_and_receive_notification_via_channel() {
  let address = SocketAddr::new( IpAddr::V4( Ipv4Addr::LOCALHOST ), client::DEFAULT_PORT );

  // Create and start an Etherdream DAC
  let dac = Etherdream::start().await;

  // Setup a channel to receive the ping notification
  let ( tx, mut rx ) = mpsc::channel::<( client::ControlSignal, client::Command )>( 16 );

  // Create and start a client
  let mut client = 
    client::Client::start_with_notifications( address, tx ).await
    .expect( "Failed to connect to Etherdream device" );

  // Send a ping
  let _ = client.ping().await;

  // Setup future to receive the ping
  let handle = timeout( Duration::from_secs( 2 ), async move {
    return match rx.recv().await.unwrap() {
      ( client::ControlSignal::Ack, client::Command::Ping ) => Ok(()),
      ( control_signal, command ) => Err( format!( "Received incorrect notification: control={:?}, command={:?}", control_signal, command ) )
    };
  });

  // Validate that we received the ping (Ok())
  assert_eq!( Ok(()), handle.await.unwrap() );

  // Shut it down
  let _ = dac.shutdown().await;
}