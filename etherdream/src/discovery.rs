//! Discovery: Tools to discover Etherdream devices on a network.
use std::collections::HashMap;
use std::io;
use std::net::{ IpAddr, Ipv4Addr, SocketAddr };
use std::sync::Arc;
use std::time::Instant;

use futures::stream::StreamExt;
use tokio::net::UdpSocket;
use tokio::sync::{ mpsc, RwLock, RwLockReadGuard, RwLockWriteGuard };
use tokio_util::bytes::BytesMut;
use tokio_util::codec::Decoder;
use tokio_util::sync::CancellationToken;
use tokio_util::udp::UdpFramed;

use crate::device_info::DeviceInfo;
use crate::protocol;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Broadcast

pub struct Broadcast {
  device_info: DeviceInfo,
  inserted_at: Instant,
  state: protocol::State,
  updated_at: Instant
}

impl Broadcast {
  fn new( device_info: DeviceInfo, state: protocol::State ) -> Self {
    Self{ device_info, inserted_at: Instant::now(), state, updated_at: Instant::now() }
  }

  pub fn device_info( &self ) -> &DeviceInfo { &self.device_info }
  pub fn inserted_at( &self ) -> &Instant { &self.inserted_at }
  pub fn state( &self ) -> &protocol::State { &self.state }
  pub fn updated_at( &self ) -> &Instant { &self.updated_at }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Builder

pub struct Builder {
  address: SocketAddr,
  device_info_tx: Option<mpsc::Sender<DeviceInfo>>,
  registry: Registry
}

impl Builder {
  /// Creates a `Discovery` builder.
  pub fn new() -> Self { Self::with_registry( Registry::default() ) }

  /// Creates a `Discovery` builder with a user-provided `Registry`.
  pub fn with_registry( registry: Registry ) -> Self {
    Self{
      address: SocketAddr::new( IpAddr::V4( Ipv4Addr::UNSPECIFIED ), protocol::BROADCAST_PORT ),
      device_info_tx: None,
      registry
    }
  }

  /// Assign a custom socket address that the `Discovery` server will listen
  /// for Etherdream broadcast messages from. Useful for testing.
  pub fn address( mut self, address: SocketAddr ) -> Self {
    self.address = address;
    self
  }

  /// Assign a channel that will receive a single `DeviceInfo` message for each
  /// new device.
  pub fn notify( mut self, device_info_tx: mpsc::Sender<DeviceInfo> ) -> Self {
    self.device_info_tx = Some( device_info_tx );
    self
  }

  /// Starts the `Discovery` task.
  pub async fn listen( self ) -> Result<Discovery,io::Error>
  {
    let shutdown_token = CancellationToken::new();

    let socket = UdpSocket::bind( self.address ).await?;
    let local_address = socket.local_addr()?;

    tokio::spawn({
      let registry = self.registry.clone();
      let shutdown_token = shutdown_token.child_token();
      async move { shutdown_token.run_until_cancelled( do_listen( socket, registry, self.device_info_tx ) ).await; }
    });

    Ok( Discovery{
      address: local_address,
      shutdown_token
    } )
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Discovery Server

pub struct Discovery {
  // The local socket address that the discovery service is listening on.
  address: SocketAddr,
  // The cancellation token used to shut down the discovery server.
  shutdown_token: CancellationToken
}

impl Discovery {
  /// Returns the local socket address that the discovery server is bound to.
  pub fn address( &self ) -> &SocketAddr { &self.address }

  /// Shuts down the discovery server and consumes `self`.
  pub fn shutdown( self ) { self.shutdown_token.cancel(); }
}

impl Drop for Discovery {
  fn drop( &mut self ) { self.shutdown_token.cancel(); }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Read-only Registry

#[derive( Default )]
pub struct Registry {
  inner: Arc<RwLock<HashMap<SocketAddr,Broadcast>>>
}

impl Registry {
  /// Returns a read-only guard of the discovery registry. This function will
  /// panic if called from an async context.
  pub fn blocking_read( &'_ self ) -> RwLockReadGuard<'_,HashMap<SocketAddr,Broadcast>> {
    self.inner.blocking_read()
  }

  /// Returns a read-only guard of the discovery registry.
  pub async fn read( &'_ self ) -> RwLockReadGuard<'_,HashMap<SocketAddr,Broadcast>> {
    self.inner.read().await
  }

  // Private-function, for use by the Discovery service.
  async fn write( &'_ self ) -> RwLockWriteGuard<'_,HashMap<SocketAddr,Broadcast>> {
    self.inner.write().await
  }
}

impl Clone for Registry {
  fn clone( &self ) -> Self {
    Self{ inner: self.inner.clone() }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Listen Handler

async fn do_listen(
  socket: UdpSocket,
  registry: Registry,
  device_info_tx: Option<mpsc::Sender<DeviceInfo>>
)
  -> Result<(),io::Error>
{
  let mut framed = UdpFramed::new( socket, BroadcastDecoder{} );

  loop {
    if let Some( frame ) = framed.next().await {
      match frame {
        Ok( ( ( intrinsics, state ), address ) ) => {
          let mut guard = registry.write().await;
          if let Some( broadcast ) = guard.get_mut( &address ) {
            broadcast.updated_at = Instant::now();
          } else {
            let client_addr = SocketAddr::new( address.ip(), protocol::CLIENT_PORT );
            let device_info = DeviceInfo::new( client_addr, intrinsics );

            guard.insert( address, Broadcast::new( device_info, state ) );

            if let Some( device_info_tx ) = device_info_tx.as_ref() {
              let _ = device_info_tx.send( device_info ).await;
            }
          }
        }
        Err( e ) => {
          eprintln!( "Error receiving discovery broadcast: {}", e );
        }
      }
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Broadcast Decoder

struct BroadcastDecoder;

impl Decoder for BroadcastDecoder {
  type Item = ( protocol::Intrinsics, protocol::State );
  type Error = io::Error;

  fn decode( &mut self, buf: &mut BytesMut ) -> Result<Option<Self::Item>, Self::Error> {
    if buf.len() < protocol::BROADCAST_BYTES_SIZE {
      Ok( None )
    } else {
      let broadcast_bytes = buf.split_to( protocol::BROADCAST_BYTES_SIZE );

      let intrinsics = protocol::Intrinsics::from_bytes( &broadcast_bytes[..protocol::INTRINSIC_BYTES_SIZE] );
      let state = protocol::State::from_bytes( &broadcast_bytes[protocol::INTRINSIC_BYTES_SIZE..] );
      Ok( Some( ( intrinsics, state ) ) )
    }
  }
}