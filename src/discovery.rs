//! A service to discover Etherdream DAC's on a network.
use std::collections::HashMap;
use std::io;
use std::net::{ IpAddr, Ipv4Addr, SocketAddr };

use tokio::net::UdpSocket;
use tokio::sync::mpsc::Sender;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::constants::*;
use crate::device;

type DeviceMap = HashMap<SocketAddr,device::Device>;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Discovery Server

pub struct Server {
  /// The local socket address that the server is listening on.
  address: SocketAddr,

  /// The tokio join handle that owns the asynchronous listening task.
  handle: JoinHandle<Result<(),io::Error>>,

  /// The cancellation token used to shut down the discovery server.
  shutdown_token: CancellationToken
}

impl Server {
  /// Starts the discovery server and listens on the Etherdream protocol
  /// defined broadcast port (`7654`).
  pub async fn serve( device_tx: Sender<device::Device> ) -> Result<Self,io::Error> {
    Self::serve_with_address(
      SocketAddr::new( IpAddr::V4( Ipv4Addr::UNSPECIFIED ), ETHERDREAM_BROADCAST_PORT ),
      device_tx
    ).await
  }

  /// Starts the discovery server and listens on a user-provided socket address.
  pub async fn serve_with_address( address: SocketAddr, device_tx: Sender<device::Device> ) -> Result<Self,io::Error> {
    let shutdown_token = CancellationToken::new();

    let socket = UdpSocket::bind( address ).await?;
    let local_address = socket.local_addr()?;

    let handle = tokio::spawn({
      let shutdown_token = shutdown_token.child_token();

      async move {
        tokio::select!{
          _ = shutdown_token.cancelled() => { Ok(()) },
          result = do_listen( socket, device_tx ) => result
        }
      }
    });

    Ok( Self{
      address: local_address,
      handle,
      shutdown_token
    })
  }

  /// Returns the local socket address that the discovery server is listening on.
  pub fn address( &self ) -> SocketAddr {
    self.address
  }

  /// Shuts down the discovery server and consumes `self`.
  pub async fn shutdown( self ) {
    self.shutdown_token.cancel();
    let _ = self.handle.await;
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Listen Handler

async fn do_listen( socket: UdpSocket, device_tx: Sender<device::Device> ) -> Result<(),io::Error>
{
  let mut buf = [0u8; ETHERDREAM_BROADCAST_BYTES];
  let mut registry = DeviceMap::new();
    
  loop {
    let ( _length, address ) = socket.recv_from( &mut buf ).await?;

    let device = device::Device::from_bytes( SocketAddr::new( address.ip(), ETHERDREAM_CLIENT_PORT ), &buf );

    // Insert the device into the registry. Only broadcast the device if it is
    // the first time the server has seen it.
    if let None = registry.insert( address, device ) {
      let _ = device_tx.send( device ).await;
    }
  }
}