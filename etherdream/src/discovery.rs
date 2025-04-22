use std::net::{ IpAddr, Ipv4Addr, SocketAddr };

use tokio::net::UdpSocket;

use crate::device::Device;

pub const BROADCAST_PORT: u16 = 7654;


// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Discovery Server

pub struct Server {
  devices: Vec<SocketAddr>,
  host: IpAddr,
  limit: usize,
}

impl Server {
  pub fn new() -> Self {
    return Self{ 
      devices: Vec::with_capacity( 16 ),
      host: IpAddr::V4( Ipv4Addr::UNSPECIFIED ),
      limit: 0
    };
  }

  pub fn host( &mut self, host: IpAddr ) -> &mut Self {
    self.host = host;
    return self;
  }

  pub fn limit( &mut self, limit: usize ) -> &mut Self {
    self.limit = limit;
    return self;
  }

  pub async fn serve<T>( &mut self, callback_fn: T ) -> std::io::Result<()>
    where 
      T: Fn( &SocketAddr, &Device ) + Send + 'static
  {
      let mut buffer = [0 as u8; 36];

      let address = SocketAddr::new( self.host, BROADCAST_PORT );
      let socket = UdpSocket::bind( address ).await?;
    
      while let Ok(( _length, address )) = socket.recv_from( &mut buffer ).await {
        if ! self.devices.contains( &address ) {
          self.devices.push( address );

          let device = Device::from_bytes( buffer );
          callback_fn( &address, &device );

          // Break if the user-provided limit has been exceeded
          if self.limit > 0 && self.devices.len() >= self.limit  {
            break;
          }
        }
      }

      return Ok(());
  }
}