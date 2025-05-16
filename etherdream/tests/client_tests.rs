use std::io;
use std::net::{ IpAddr, Ipv4Addr, SocketAddr };

use tokio::io::{ AsyncReadExt, AsyncWriteExt };
use tokio::net;
use tokio::task;

use etherdream::client;

fn start_etherdream() -> task::JoinHandle<io::Result<()>> {
  return tokio::spawn( async move { 
    let listener = net::TcpListener::bind( "127.0.0.1:7765" ).await?;
    
    let mut buf = [0 as u8; 1024];

    loop {
      let ( mut socket, _ ) = listener.accept().await?;
      let _n = socket.read( &mut buf ).await.expect("failed to read data from socket");

      if buf[0]  == b"p"[0] {
        let _ = socket.write( b"ap" ).await.expect( "failed to write data to socket" );
      }

      break;
    }

    return Ok(());
  });
}

#[tokio::test]
async fn sends_a_ping_and_receives_a_callback() {
  let dac = start_etherdream();

  // Create and start client
  let address = SocketAddr::new( IpAddr::V4( Ipv4Addr::LOCALHOST ), client::DEFAULT_PORT );
  let client_builder = client::Builder::new( address );
  let mut client = client_builder.start().await;

  client.ping();

  let _ = dac.await;
  let _ = client.stop().await;
}