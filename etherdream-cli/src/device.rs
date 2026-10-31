use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::{ Arc, RwLock };

use etherdream::protocol;

use crate::{ sequencer, tracks };

pub struct Device {
  info: etherdream::DeviceInfo,
  generator: Option<etherdream::Generator>,
  state: Arc<RwLock<sequencer::State>>
}

impl Device {
  pub fn new( info: etherdream::DeviceInfo ) -> Self {
    let state = Arc::new( RwLock::new( sequencer::State::default() ) );
    Self{ generator: None, info, state }
  }

  pub fn info( &self ) -> &etherdream::DeviceInfo { &self.info }

  pub async fn connect( &mut self ) -> Result<(), etherdream::client::Error> {
    if self.generator.is_some() { return Ok( () ); }

    match etherdream::connect( self.info ).await {
      Ok( client ) => {
        // TODO: not sure this required...
        let mut tracks: BTreeMap<String, Box<dyn sequencer::Track>> = BTreeMap::new();

        for definition in tracks::DEFINITIONS.iter() {
          tracks.insert( definition.id.clone(), ( definition.create_fn )() );
        }

        let exe = sequencer::Executor::new( self.state.clone(), tracks );
        self.generator = Some( etherdream::make_generator( client, Box::new( exe ) ) );
        Ok( () )
      }
      Err( err ) => Err( err )
    }
  }

  pub async fn disconnect( &mut self ) {
    if let Some( generator ) = self.generator.take() && let Ok( client ) = generator.into_client().await {
      client.disconnect().await
    }
  }

  pub fn address( &self ) -> SocketAddr { SocketAddr::new( self.info.ip(), protocol::CLIENT_PORT ) }

  pub fn is_connected( &self ) -> bool { self.generator.is_some() }

  pub fn state( &self ) -> Option<tokio::sync::watch::Ref<'_, etherdream::State>> {
    self.generator.as_ref().map(| g | g.state() )
  }

  pub fn play( &self, track_id: &'static str ) {
    if self.is_connected() && let Ok( mut state ) = self.state.write() {
      state.set_current( Some( track_id ) );
    }
  }

  ///
  pub fn is_playing( &self ) -> bool {
    if let Ok( state ) = self.state.read() {
      state.current().is_some()
    } else {
      false
    }
  }

  pub fn stop( &self ) {
    if let Ok( mut state ) = self.state.write() {
      state.set_current( None );
    }
  }
}