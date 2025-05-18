use std::io;
use std::net::{ IpAddr, Ipv4Addr, SocketAddr };

use tokio::io::{ AsyncReadExt, AsyncWriteExt };
use tokio::net;
use tokio::task;

use etherdream::client;

async fn start_etherdream( address: SocketAddr ) -> io::Result<task::JoinHandle<io::Result<()>>> {
  println!( "> DAC: binding..." );
  let listener = net::TcpListener::bind( address ).await?;
    
  let handle = tokio::spawn( async move {
    let mut buf = [0 as u8; 1024];

    println!( "> DAC: accepting..." );
    let ( mut socket, _ ) = listener.accept().await?;
    println!( "> DAC: reading..." );

    loop {
      let _n = socket.read( &mut buf ).await.expect("failed to read data from socket");
      println!( "> DAC: received..." );

      if buf[0]  == b"p"[0] {
        println!( "> DAC: sending ping response..." );
        let _ = socket.write( b"ap" ).await.expect( "failed to write data to socket" );
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
  let dac = start_etherdream( address ).await.unwrap();

  // Create and start client
  let mut client = client::Builder::new( address ).start().await.expect( "Failed to connect..." );

  // wait for dac connected;

  // Send a ping
  client.ping();
  println!( "> ping..." );

  let _ = dac.await;
  let _ = client.stop().await;

  tokio::time::sleep( tokio::time::Duration::from_secs( 2 ) ).await;
}