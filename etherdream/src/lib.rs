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
pub use generator::Generator;

/// Starts a discovery service that will listen for Etherdream network device's
/// on the local network. The service will shut down:
///   (A) When `Discovery::shutdown` is called, or...
///   (B) When the Discovery instance is dropped
///
/// Use `discovery::Builder` for further service customization.
pub async fn discover( registry: discovery::Registry ) -> Result<discovery::Service,io::Error> {
  discovery::Discovery::with_registry( registry ).listen().await
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