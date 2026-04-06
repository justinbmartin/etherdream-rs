//! Etherdream
//!
//! Tools to discover, connect, and write laser data to Etherdream devices.
mod circular_buffer;
pub mod client;
pub mod protocol;
pub mod device_info;
pub mod discovery;
pub mod generator;

#[cfg(test)]
mod tests;

// Convenience exports
pub use client::{ Client, State };
pub use device_info::DeviceInfo;
pub use discovery::Server as Discovery;
pub use generator::Generator;

/// Creates an Etherdream `Client`.
pub async fn connect( device_info: DeviceInfo ) -> Result<Client,client::Error> {
  client::Builder::new( device_info ).connect().await
}

/// Creates an Etherdream `Client` with a custom point buffer capacity.
pub async fn connect_with_capacity( device_info: DeviceInfo, capacity: usize ) -> Result<Client,client::Error> {
  client::Builder::new( device_info )
    .capacity( capacity )
    .connect().await
}

/// Creates a `Generator` from an existing `Client` and `executor`.
pub fn generator( client: Client, executor: Box<dyn generator::Executable> ) -> Generator {
  Generator::new( client, executor )
}