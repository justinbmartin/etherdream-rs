use etherdream::device;

// A convenience device builder for testing.
#[derive( Clone, Copy )]
pub struct DeviceBuilder {
  bytes: [u8; device::DEVICE_BYTES_SIZE]
}

impl DeviceBuilder {
  // Creates a new device builder with all bytes set to zero.
  pub fn new() -> Self { 
    return Self{ bytes: [0u8; device::DEVICE_BYTES_SIZE] };
  }

  // Creates a new device builder with typical default values for an Etherdream DAC.
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
      .version( 0, 1 );
  }

  // Returns a `Device` from the bytes constructed. Consumes self.
  pub fn to_device( self ) -> device::Device {
    return device::Device::from_bytes( self.bytes );
  }

  pub fn buffer_capacity( mut self, capacity: u16 ) -> Self {
    self.bytes[10..12].copy_from_slice( &capacity.to_le_bytes() );
    return self;
  }

  pub fn light_engine_state( mut self, state: device::LightEngineState ) -> Self {
    self.bytes[17] = ( state as u8 ).to_le();
    return self;
  }

  pub fn mac_address<T: Into<[u8;6]>>( mut self, address: T ) -> Self {
    self.bytes[0..6].copy_from_slice( &address.into() );
    return self;
  }

  pub fn max_points_per_second( mut self, points_per_second: u32 ) -> Self {
    self.bytes[12..16].copy_from_slice( &points_per_second.to_le_bytes() );
    return self;
  }

  pub fn playback_state( mut self, state: device::PlaybackState ) -> Self {
    self.bytes[18] = state as u8;
    return self;
  }

  pub fn points_lifetime( mut self, count: u32 ) -> Self {
    self.bytes[32..36].copy_from_slice( &count.to_le_bytes() );
    return self;
  }

  pub fn points_per_second( mut self, points_per_second: u32 ) -> Self {
    self.bytes[28..32].copy_from_slice( &points_per_second.to_le_bytes() );
    return self;
  }

  pub fn source( mut self, source: device::Source ) -> Self {
    self.bytes[19] = source as u8;
    return self;
  }

  pub fn version( mut self, hardware: u16, software: u16 ) -> Self {
    self.bytes[6..8].copy_from_slice( &hardware.to_le_bytes() );
    self.bytes[8..10].copy_from_slice( &software.to_le_bytes() );
    return self;
  }
}