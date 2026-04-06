//! A service to discover Etherdream DAC's on a network.
use std::collections::HashMap;
use std::io;
use std::net::{ IpAddr, Ipv4Addr, SocketAddr };

use futures::stream::StreamExt;
use tokio::net::UdpSocket;
use tokio::sync::mpsc::Sender;
use tokio::task::JoinHandle;
use tokio_util::bytes::BytesMut;
use tokio_util::codec::Decoder;
use tokio_util::sync::CancellationToken;
use tokio_util::udp::UdpFramed;

use crate::device_info::DeviceInfo;
use crate::protocol;

type DeviceMap = HashMap<SocketAddr,DiscoveredDeviceInfo>;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - Discovered Device Info

//
#[derive( Clone )]
pub struct DiscoveredDeviceInfo {
  device_info: DeviceInfo,
  state: protocol::State
}

impl DiscoveredDeviceInfo {
  pub fn info( &self ) -> &DeviceInfo { &self.device_info }
  pub fn state( &self ) -> &protocol::State { &self.state }
}

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
  pub async fn serve( device_tx: Sender<DiscoveredDeviceInfo> ) -> Result<Self,io::Error> {
    Self::serve_with_address(
      SocketAddr::new( IpAddr::V4( Ipv4Addr::UNSPECIFIED ), protocol::BROADCAST_PORT ),
      device_tx
    ).await
  }

  /// Starts the discovery server and listens on a user-provided socket address.
  pub async fn serve_with_address( address: SocketAddr, device_tx: Sender<DiscoveredDeviceInfo> ) -> Result<Self,io::Error> {
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

async fn do_listen( socket: UdpSocket, device_tx: Sender<DiscoveredDeviceInfo> ) -> Result<(),io::Error>
{
  let mut framed = UdpFramed::new( socket, BroadcastDecoder{} );
  let mut registry = DeviceMap::new();

  loop {
    if let Some( frame ) = framed.next().await {
      match frame {
        Ok( ( ( intrinsics, state ), address ) ) => {
          let device_info = DeviceInfo::new( address, intrinsics );
          let discovered_device = DiscoveredDeviceInfo{ device_info, state };

          // Insert the device into the registry. Only broadcast the device if
          // it is the first time the server has seen it.
          if let None = registry.insert( address, discovered_device.clone() ) {
            let _ = device_tx.send( discovered_device ).await;
          }
        }
        Err( e ) => {
          eprintln!( "Error receiving datagram: {}", e );
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