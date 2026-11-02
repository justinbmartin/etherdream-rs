use std::collections::HashMap;
use std::hash::Hash;
use std::net::SocketAddr;

use etherdream::protocol;

use crate::executors;
use crate::sequencer::{ self, Sequencer };

pub struct Device<T> {
  info: etherdream::DeviceInfo,
  sequencer: Option<Sequencer<T>>
}

impl<T> Device<T>
  where T: 'static + Copy + Default + Eq + Hash + Send + Sync
{
  pub fn new( info: etherdream::DeviceInfo ) -> Self {
    Self{ info, sequencer: None }
  }

  pub fn info( &self ) -> &etherdream::DeviceInfo { &self.info }

  pub async fn connect( &mut self ) -> Result<(), etherdream::client::Error> {
    if self.sequencer.is_some() { return Ok( () ); }

    match etherdream::connect( self.info ).await {
      Ok( client ) => {
        // TODO: not sure this required...
        // NEXT: this should either be a behavior, that i can load into, or
        // manually setup one-by-one, and figured out later;
        let mut tracks: HashMap<T, Box<dyn sequencer::Track>> = HashMap::new();

        for definition in executors::DEFINITIONS.iter() {
          tracks.insert( definition.id, ( definition.create_fn )() );
        }

        let sequencer = Sequencer::new( client, tracks );

        self.sequencer = Some( sequencer );
        Ok( () )
      }
      Err( err ) => Err( err )
    }
  }

  pub async fn disconnect( &mut self ) {
    if let Some( sequencer ) = self.sequencer.take() {
      let _ = sequencer.disconnect().await;
    }
  }

  pub fn address( &self ) -> SocketAddr { SocketAddr::new( self.info.ip(), protocol::CLIENT_PORT ) }

  pub fn is_connected( &self ) -> bool { self.sequencer.is_some() }

  pub fn play( &mut self, track_id: Option<T> ) {
    self.sequencer.as_mut().map(| s | s.play( track_id ) );
  }

  ///
  pub fn is_playing( &self ) -> bool {
    if let Some( sequencer ) = self.sequencer.as_ref() {
      sequencer.is_playing()
    } else {
      false
    }
  }

  ///
  pub fn stop( &mut self ) { self.play( None ) }
}