use std::net::{ IpAddr, Ipv4Addr, SocketAddr };

use tokio::net::UdpSocket;

use crate::{device, Device};

pub const BROADCAST_PORT: u16 = 7654;


// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Discovery Server

pub struct Server {
  /// A list of disvocered devices.
  devices: Vec<SocketAddr>,

  /// The local host that the server will listen on. Defaults to `0.0.0.0`.
  host: IpAddr,

  /// If greater than zero, the maximum number of devices that this server
  /// will listen for.
  limit: usize,
}

impl Server {
  /// Creates a new discovery server. Must call `serve().await` to start.
  pub fn new() -> Self {
    return Self{ 
      devices: Vec::with_capacity( 32 ),
      host: IpAddr::V4( Ipv4Addr::UNSPECIFIED ),
      limit: 0
    };
  }

  // Will override the default host (0.0.0.0) that the server listens on.
  pub fn host( &mut self, host: IpAddr ) -> &mut Self {
    self.host = host;
    return self;
  }

  // Limits the number of devices that the server will discover. Useful for testing.
  pub fn limit( &mut self, limit: usize ) -> &mut Self {
    self.limit = limit;
    return self;
  }

  pub async fn serve<T>( &mut self, callback_fn: T ) -> std::io::Result<()>
    where 
      T: Fn( SocketAddr, Device ) + Send + 'static
  {
      let mut buffer = [0u8; device::DEVICE_BYTES_SIZE];

      let address = SocketAddr::new( self.host, BROADCAST_PORT );
      let socket = UdpSocket::bind( address ).await?;
    
      while let Ok(( _length, address )) = socket.recv_from( &mut buffer ).await {
        if ! self.devices.contains( &address ) {
          self.devices.push( address );

          let device = Device::from_bytes( buffer );
          callback_fn( address, device );

          // Break if the user-provided limit has been exceeded
          if self.limit > 0 && self.devices.len() >= self.limit  {
            break;
          }
        }
      }

      return Ok(());
  }
}