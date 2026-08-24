//! Etherdream
//!
//! Tools to discover, connect, and write laser data to Etherdream devices.
mod circular_buffer;
pub mod client;
pub mod protocol;
pub mod device_info;
pub mod discovery;
pub mod generator;

use std::io;

// Convenience exports
pub use client::{ Client, State };
pub use device_info::DeviceInfo;
pub use discovery::{ DiscoveredDeviceInfo, Discovery };
pub use generator::Generator;

/// Starts a server that discovers Etherdream network device's. The server will
/// shut down when the `discovery::Server` is dropped.
///
/// Each unique device will be published to the user-provided `tx` a single
/// time.
pub async fn discover() -> Result<Discovery,io::Error> {
  Discovery::listen().await
}

/// Connects to an Etherdream network device using the provided `DeviceInfo`,
/// returning a `Client` on success.
pub async fn connect( device_info: impl Into<DeviceInfo> )
  -> Result<Client,client::Error>
{
  client::Builder::new( device_info.into() ).connect().await
}

/// Makes a `Generator` from an existing `Client` and user-provided
/// `generator::Executable`.
pub fn make_generator( client: Client, executor: Box<dyn generator::Executable> )
  -> Generator
{
  Generator::new( client, executor )
}