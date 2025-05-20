use std::io;
use std::net::{ IpAddr, Ipv4Addr, SocketAddr };

use tokio::io::{ AsyncReadExt, AsyncWriteExt };
use tokio::net;
use tokio::task;

use etherdream::client;

async fn start_etherdream( _address: SocketAddr ) -> io::Result<task::JoinHandle<io::Result<()>>> {
  println!( "> DAC: binding..." );
  let listener = net::TcpListener::bind( "127.0.0.1:7765" ).await?;
    
  let handle = tokio::spawn( async move {
    let mut buf = [0 as u8; 1];

    println!( "> DAC: accepting..." );
    let ( mut stream, remote ) = listener.accept().await?;
    dbg!( remote );
    
    loop {
      println!( "> DAC: reading..." );
      dbg!( &stream );
      let _n = stream.read( &mut buf ).await?;
      println!( "> DAC: received..." );

      if buf[0]  == b"p"[0] {
        println!( "> DAC: sending ping response..." );
        let _ = stream.write( b"ap" ).await?;
      }

      break;
    }

    return Ok(());
  });

  return Ok( handle );
}

#[tokio::test]
async fn sends_a_ping_and_receives_a_callback() {
  let _dac_ready = std::sync::Arc::new( parking_lot::RwLock::new( false ) );

  let address = SocketAddr::new( IpAddr::V4( Ipv4Addr::LOCALHOST ), client::DEFAULT_PORT );
  let dac = start_etherdream( address ).await.unwrap();

  // Create and start client
  let mut client = client::Builder::new( address ).start().await.expect( "Failed to connect..." );

  // Send a ping
  client.ping();
  println!( "> ping..." );

  let _ = dac.await;
  let _ = client.stop().await;

  tokio::time::sleep( tokio::time::Duration::from_secs( 2 ) ).await;
}