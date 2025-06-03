
use std::collections::HashMap;
use std::io;
use std::net::{ IpAddr, Ipv4Addr, SocketAddr };

use tokio::net::UdpSocket;
use tokio::task::JoinHandle;
use tokio::time::Duration;

use crate::{device, Device};

pub const BROADCAST_PORT: u16 = 7654;

type DeviceMap = HashMap<SocketAddr,Device>;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Connection

pub struct Connection {
  address: SocketAddr,
  handle: JoinHandle<io::Result<DeviceMap>>
}

impl Connection {
  pub fn address( &self ) -> SocketAddr {
    return self.address;
  }
}

impl Drop for Connection {
  fn drop( &mut self ) {
    self.handle.abort();
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - Discovery Server Builder

#[derive( Clone, Copy )]
pub struct Server<T> 
  where T: Fn( SocketAddr, Device ) + Send + Sync + 'static
{
  /// The local host that the server will listen on. Defaults to `0.0.0.0::7654`.
  address: SocketAddr,

  //
  callback_fn: T,

  /// If greater than zero, the maximum number of devices that this server
  /// will listen for.
  limit: usize,

  // The optional length of time this server will listen for.
  duration: Option<Duration>
}

impl<T> Server<T> 
  where T: Fn( SocketAddr, Device ) + Send + Sync + 'static
{
  /// Creates a new discovery server. Must call `serve().await` to start.
  pub fn new( callback_fn: T ) -> Self 
    where T: Fn( SocketAddr, Device ) + Send + 'static
  {
    return Self{
      address: SocketAddr::new( IpAddr::V4( Ipv4Addr::UNSPECIFIED ), BROADCAST_PORT ),
      callback_fn: callback_fn,
      duration: None,
      limit: 0
    };
  }

  // Will override the default host that the server listens on.
  pub fn address( mut self, address: SocketAddr ) -> Self {
    self.address = address;
    return self;
  }

  // Limits the number of devices that the server will discover. Useful for testing.
  pub fn limit( mut self, limit: usize ) -> Self {
    self.limit = limit;
    return self;
  }

  // Limits the time that the server will run for. Useful for testing.
  pub fn duration( mut self, duration: Duration ) -> Self {
    self.duration = Some( duration );
    return self;
  }

  //
  pub async fn serve( self ) -> io::Result<Connection> {
    let socket = UdpSocket::bind( self.address ).await?;
    let addr = socket.local_addr()?;

    let handle =
      if let Some( duration ) = self.duration {
        tokio::spawn( async move {
          tokio::time::timeout( duration, do_listen( socket, self.callback_fn, self.limit ) ).await?
        })
      } else {
        tokio::spawn( do_listen( socket, self.callback_fn, self.limit ) )
      };

    return Ok( Connection{
      address: addr,
      handle: handle
    });
  }
}

//

async fn do_listen<T>( socket: UdpSocket, callback_fn: T, limit: usize ) -> io::Result<HashMap<SocketAddr,Device>> 
  where T: Fn( SocketAddr, Device ) + Send + Sync + 'static
{
  let mut buffer = [0u8; device::DEVICE_BYTES_SIZE];
  let mut devices: HashMap<SocketAddr,Device> = HashMap::new();
    
  loop {
    let ( _length, address ) = socket.recv_from( &mut buffer ).await?;
        
    if ! devices.contains_key( &address ) {
      let device = Device::from_bytes( buffer );
      devices.insert( address, device );

      callback_fn( address, device );

      // Break if limit has been met
      if limit > 0 && devices.len() >= limit  {
        break;
      }
    }
  }

  return Ok( devices );
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Functions

pub async fn server<T>( callback_fn: T ) -> io::Result<Connection>
  where T: Fn( SocketAddr, Device ) + Send + Sync + 'static
{
  return Server::new( callback_fn ).serve().await;
}