use std::net::SocketAddr;

use crate::protocol::{ Intrinsics, MacAddress, Version };

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Device Info

/// A model used to describe a remote device's (DAC) address and intrinsic
/// properties. Most often provided by a `Discovery` service.
#[derive( Clone, Copy, Debug )]
pub struct DeviceInfo {
  address: SocketAddr,
  intrinsics: Intrinsics
}

impl DeviceInfo {
  /// Creates a new `DeviceInfo` from a socket address and device intrinsics.
  pub fn new( address: SocketAddr, intrinsics: Intrinsics ) -> Self {
    Self{ address, intrinsics }
  }

  /// Returns the socket address that the device is communicating on.
  #[inline]
  pub fn address( &self ) -> &SocketAddr { &self.address }

  /// Returns the maximum number of points the device can buffer.
  #[inline]
  pub fn buffer_capacity( &self ) -> usize { self.intrinsics.buffer_capacity as usize }

  /// Returns the MAC address for the device.
  #[inline]
  pub fn mac_address( &self ) -> MacAddress { self.intrinsics.mac_address }

  /// Returns the maximum number of points this device can process per second.
  #[inline]
  pub fn max_points_per_second( &self ) -> usize { self.intrinsics.max_points_per_second as usize }

  /// Returns the hardware and software version installed on the device.
  #[inline]
  pub fn version( &self ) -> Version { self.intrinsics.version }
}