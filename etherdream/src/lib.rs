//! Etherdream
//!
//! Tools to discover, connect, and write laser data to Etherdream devices.
use std::io;

mod circular_buffer;
pub mod client;
pub mod protocol;
pub mod device_info;
pub mod discovery;
pub mod generator;

// Convenience exports
pub use client::{ Client, State };
pub use device_info::DeviceInfo;
pub use discovery::Discovery;
pub use generator::Generator;

/// Starts a server that discovers Etherdream network device's. The server will
/// shut down:
///   (A) When `Discovery::shutdown` is called, or...
///   (B) When the Discovery instance is dropped
///
/// Use `Discovery::recv` to consume all discovered devices. Each unique device
/// (by remote socket address) will be published to `Discovery::recv` exactly
/// one time.
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