use std::sync::LazyLock;

use crate::sequencer;

mod beam;
mod common;
mod windmill;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Track Ids

#[derive( Clone, Copy, Debug, Eq, Hash, PartialEq )]
pub enum TrackId {
  Beam,
  Windmill
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Track

#[derive( Clone )]
pub struct Track {
  id: TrackId,
  name: String
}

impl Track {
  pub fn name( &self ) -> &str { &self.name }
}

impl sequencer::Track for Track {
  type Id = TrackId;

  fn id( &self ) -> &TrackId { &self.id }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Track List

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
