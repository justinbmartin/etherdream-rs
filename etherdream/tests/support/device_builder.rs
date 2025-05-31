use etherdream::{device, Device};

// A convenience builder to help make devices for testing.
#[derive( Clone, Copy )]
pub struct DeviceBuilder {
  bytes: [u8; device::DEVICE_BYTES_SIZE]
}

impl DeviceBuilder {
  // Creates a new device builder with all bytes set to zero.
  pub fn new() -> Self { 
    return Self{ bytes: [0u8; device::DEVICE_BYTES_SIZE] };
  }

  // Creates a new device builder with typical default values.
  pub fn default() -> Self {
    return Self::new()
      .buffer_capacity( 1024 )
      .light_engine_state( device::LightEngineState::Ready )
      .mac_address([ 0, 1, 2, 3, 4, 5 ])
      .max_points_per_second( 128 )
      .playback_state( device::PlaybackState::Idle )
      .points_lifetime( 16384 )
      .points_per_second( 128 )
      .source( device::Source::Ilda )
      .version( 0, 1 )
      .to_owned();
  }

  // Returns a `Device` from the bytes constructed. Consumes self.
  pub fn to_device( self ) -> Device {
    return Device::from_bytes( self.bytes );
  }

  pub fn buffer_capacity( &mut self, capacity: u16 ) -> &mut Self {
    self.bytes[10..12].copy_from_slice( &capacity.to_le_bytes() );
    return self;
  }

  pub fn light_engine_state( &mut self, state: device::LightEngineState ) -> &mut Self {
    self.bytes[13] = ( state as u8 ).to_le();
    return self;
  }

  pub fn mac_address<T: Into<[u8;6]>>( &mut self, address: T ) -> &mut Self {
    self.bytes[0..6].copy_from_slice( &address.into() );
    return self;
  }

  pub fn max_points_per_second( &mut self, points_per_second: u32 ) -> &mut Self {
    self.bytes[10..14].copy_from_slice( &points_per_second.to_le_bytes() );
    return self;
  }

  pub fn playback_state( &mut self, state: device::PlaybackState ) -> &mut Self {
    self.bytes[14] = state as u8;
    return self;
  }

  pub fn points_lifetime( &mut self, count: u32 ) -> &mut Self {
    self.bytes[14..18].copy_from_slice( &count.to_le_bytes() );
    return self;
  }

  pub fn points_per_second( &mut self, points_per_second: u32 ) -> &mut Self {
    self.bytes[14..18].copy_from_slice( &points_per_second.to_le_bytes() );
    return self;
  }

  pub fn source( &mut self, source: device::Source ) -> &mut Self {
    self.bytes[14] = source as u8;
    return self;
  }

  pub fn version( &mut self, hardware: u16, _software: u16 ) -> &mut Self {
    self.bytes[14..18].copy_from_slice( &hardware.to_le_bytes() );
    return self;
  }
}