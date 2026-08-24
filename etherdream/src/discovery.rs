//! Discovery: Tools to discover Etherdream devices on a network.
use std::collections::HashMap;
use std::io;
use std::net::{ IpAddr, Ipv4Addr, SocketAddr };

use futures::stream::StreamExt;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio_util::bytes::BytesMut;
use tokio_util::codec::Decoder;
use tokio_util::sync::CancellationToken;
use tokio_util::udp::UdpFramed;

use crate::device_info::DeviceInfo;
use crate::protocol;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - Discovered Device Info

/// Models a broadcast message from an Etherdream device as its `DeviceInfo`
/// and `protocol::State` (as received on first broadcast).
#[derive( Clone, Debug )]
pub struct DiscoveredDeviceInfo {
  device_info: DeviceInfo,
  state: protocol::State
}

impl DiscoveredDeviceInfo {
  pub fn info( &self ) -> &DeviceInfo { &self.device_info }
  pub fn state( &self ) -> &protocol::State { &self.state }
}

impl From<DiscoveredDeviceInfo> for DeviceInfo {
  fn from( discovered_device_info: DiscoveredDeviceInfo ) -> Self {
    discovered_device_info.device_info
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Discovery Server

pub struct Discovery {
  // The local socket address that the discovery server is listening on.
  address: SocketAddr,
  // Receiver for discovered devices.
  device_rx: mpsc::Receiver<( DeviceInfo, protocol::State )>,
  // The cancellation token used to shut down the discovery server.
  shutdown_token: CancellationToken
}

impl Discovery {
  /// Starts the discovery server and listens for Etherdream broadcasts on
  /// `0.0.0.0:7654`.
  pub async fn listen() -> Result<Self,io::Error>
  {
    Self::listen_with_address(
      SocketAddr::new( IpAddr::V4( Ipv4Addr::UNSPECIFIED ), protocol::BROADCAST_PORT ),
    ).await
  }

  /// Starts the discovery server and listens for Etherdream broadcasts on a
  /// user-provided socket address.
  pub async fn listen_with_address( address: SocketAddr ) -> Result<Self,io::Error>
  {
    let ( device_tx, device_rx ) = mpsc::channel::<( DeviceInfo, protocol::State )>( 16 );
    let shutdown_token = CancellationToken::new();

    let socket = UdpSocket::bind( address ).await?;
    let local_address = socket.local_addr()?;

    tokio::spawn({
      let shutdown_token = shutdown_token.child_token();
      async move { shutdown_token.run_until_cancelled( do_listen( socket, device_tx ) ).await; }
    });

    Ok( Self{
      address: local_address,
      device_rx,
      shutdown_token
    } )
  }

  /// Returns the local socket address that the discovery server is bound to.
  pub fn address( &self ) -> &SocketAddr { &self.address }

  /// Receives devices as they are discovered.
  pub async fn recv( &mut self ) -> Option<( DeviceInfo, protocol::State )> { self.device_rx.recv().await }

  /// Shuts down the discovery server and consumes `self`.
  pub fn shutdown( self ) { self.shutdown_token.cancel(); }
}

impl Drop for Discovery {
  fn drop( &mut self ) { self.shutdown_token.cancel(); }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Listen Handler

async fn do_listen( socket: UdpSocket, device_tx: mpsc::Sender<( DeviceInfo, protocol::State )> )
  -> Result<(),io::Error>
{
  let mut framed = UdpFramed::new( socket, BroadcastDecoder{} );
  let mut registry = HashMap::<SocketAddr,DeviceInfo>::new();

  loop {
    if let Some( frame ) = framed.next().await {
      match frame {
        Ok( ( ( intrinsics, state ), address ) ) => {
          if registry.contains_key( &address ) { continue; }

          // The broadcast port is not the same port that the client will
          // communicate on. Construct a `client_addr` with the broadcast
          // address, but `protocol::CLIENT_PORT`.
          let client_addr = SocketAddr::new( address.ip(), protocol::CLIENT_PORT );

          let device_info = DeviceInfo::new( client_addr, intrinsics );

          // Insert the device into the registry and broadcast it to `tx`
          registry.insert( address, device_info.clone() );
          let _ = device_tx.send( ( device_info, state ) ).await;
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