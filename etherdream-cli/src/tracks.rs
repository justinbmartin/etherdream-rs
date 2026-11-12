use std::sync::LazyLock;

use crate::sequencer;

mod beam;
mod common;
mod windmill;

/// Our defined tracks
#[derive( Clone, Copy, Debug, Eq, Hash, PartialEq )]
pub enum TrackId {
  Beam,
  Windmill
}

#[derive( Clone )]
pub struct Track {
  id: TrackId,
  name: String
}

impl sequencer::Track for Track {
  type Id = TrackId;

  fn id( &self ) -> &TrackId { &self.id }
}

impl Track {
  pub fn name( &self ) -> &str { &self.name }
}

pub const TRACKS: LazyLock<Vec<sequencer::TrackDefinition<Track>>> = LazyLock::new( ||{
  vec![
    sequencer::TrackDefinition::new(
      Track{ id: TrackId::Beam, name: String::from( "Beam" ) },
      ||{ Box::new( beam::Beam::default() ) }
    ),
    sequencer::TrackDefinition::new(
      Track{ id: TrackId::Windmill, name: String::from( "Windmill" ) },
      ||{ Box::new( windmill::Windmill::new() ) })
  ]
});
