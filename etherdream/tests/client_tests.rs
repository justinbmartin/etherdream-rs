use std::io;
use std::net::{ IpAddr, Ipv4Addr, SocketAddr };
use std::time::Duration;

use tokio::io::{ AsyncReadExt, AsyncWriteExt };
use tokio::net;
use tokio::sync;
use tokio::task;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

use etherdream::client;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Etherdream Response

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

// - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Mock Etherdream DAC

struct MockEtherdream {
  shutdown_token: CancellationToken,
  handle: task::JoinHandle<io::Result<()>>
}

impl MockEtherdream {
  async fn start( address: SocketAddr ) -> io::Result<MockEtherdream> {
    let shutdown_token = CancellationToken::new();

    // Bind to the defined Etherdream port
    let listener = net::TcpListener::bind( address ).await?;

    // Spawn a task so we can join against it when we shutdown.
    let handle = tokio::spawn({
      let shutdown_token = shutdown_token.child_token();

      async move {

        // This `select!` will run the server UNTIL a cancellation is executed.
        tokio::select!{
          _ = shutdown_token.cancelled() => { 
            Ok(())
          }

          result = async move {
            let mut buf = [0u8; 128];
      
            // An Etherdream client has connected.
            let ( mut stream, _remote ) = listener.accept().await?;
          
            // Now we listen for any data the client will send us.
            loop {
              let _ = stream.read( &mut buf ).await?;
      
              // Extract the first byte and determine what action was requested. 
              // For each action, return the expected response.
              match buf[0].into() {
                client::Command::Ping => {
                  let response = EtherdreamResponseBuilder::new( client::ControlSignal::Ack, client::Command::Ping ).to_bytes();
                  let _ = stream.write( &response ).await?;
                }
              }
            }
          } => { result }
      }
    }});
    
    return Ok( MockEtherdream{
      shutdown_token: shutdown_token,
      handle: handle
    });
  }

  async fn shutdown( self ) -> io::Result<()> {
    self.shutdown_token.cancel();
    return self.handle.await?
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Tests

#[tokio::test]
async fn send_ping_and_receive_notification_via_channel() {
  let address = SocketAddr::new( IpAddr::V4( Ipv4Addr::LOCALHOST ), client::DEFAULT_PORT );

  // Create and start a mock Etherdream DAC
  let dac = MockEtherdream::start( address ).await.unwrap();

  // Setup a channel to receive command notifications from the server
  let ( tx, mut rx ) = sync::mpsc::channel( 1 );

  let on_command_handler = move | control_signal, command | {
    let _ = match ( control_signal, command ) {
      ( client::ControlSignal::Ack, client::Command::Ping ) => tx.try_send( true ),
      _ => tx.try_send( false )
    };
  };

  // Create and start a client
  let mut client = 
    client::Client::start( address, on_command_handler ).await
    .expect( "Failed to connect to Etherdream device" );

  // Send a ping
  let _ = client.ping().await;

  // Setup future to receive the ping
  let handle = timeout( Duration::from_secs( 2 ), async move {
    return rx.recv().await;
  });

  // Validate that we received the ping (Ok())
  assert_eq!( Some( true ), handle.await.unwrap() );

  // Shut it down
  let _ = dac.shutdown().await;
}