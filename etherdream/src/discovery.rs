
use std::collections::HashMap;
use std::net::{ IpAddr, Ipv4Addr, SocketAddr };

use tokio::net::UdpSocket;

use crate::{device, Device};

pub const BROADCAST_PORT: u16 = 7654;


// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Discovery Server

#[derive( Clone, Copy )]
pub struct Server {
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

  pub async fn listen<T>( self, callback_fn: T ) -> std::io::Result<HashMap<SocketAddr,Device>>
    where 
      T: Fn( SocketAddr, Device ) + Send + 'static
  {
    let mut buffer = [0u8; device::DEVICE_BYTES_SIZE];
    let mut devices: HashMap<SocketAddr,Device> = HashMap::new();
    
    let address = SocketAddr::new( self.host, BROADCAST_PORT );
    let socket = UdpSocket::bind( address ).await?;
    
    while let Ok(( _length, address )) = socket.recv_from( &mut buffer ).await {
      if ! devices.contains_key( &address ) {
        let device = Device::from_bytes( buffer );
        devices.insert( address, device );

        callback_fn( address, device );

        // Break if limit has been met
        if self.limit > 0 && devices.len() >= self.limit  {
          break;
        }
      }
    }

    return Ok( devices );
  }
}