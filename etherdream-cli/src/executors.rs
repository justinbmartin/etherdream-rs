// This is effectively the "sequencer" configuration for this crate.
use std::fmt;
use std::sync::LazyLock;

use crate::sequencer;

mod beam;
mod common;
mod windmill;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Track Identifiers

#[derive( Clone, Copy, Debug, Eq, Hash, PartialEq )]
pub enum TrackId {
  Beam,
  Windmill
}

impl fmt::Display for TrackId {
  fn fmt( &self, f: &mut fmt::Formatter<'_> ) -> fmt::Result {
    match self {
      Self::Beam => write!( f, "Beam" ),
      Self::Windmill => write!( f, "Windmill" )
    }
  }
}

#[derive( Clone )]
pub struct TrackInfo {
  id: TrackId,
  name: String
}

impl sequencer::TrackInfo for TrackInfo {
  type TrackId = TrackId;

  fn id( &self ) -> &TrackId { &self.id }
}

impl TrackInfo {
  pub fn name( &self ) -> &str { &self.name }
}

pub const TRACKS: LazyLock<Vec<sequencer::TrackDefinition<TrackInfo>>> = LazyLock::new( ||{
  vec![
    sequencer::TrackDefinition::new(
      TrackInfo{ id: TrackId::Beam, name: String::from( "Beam" ) },
      ||{ Box::new( beam::Beam::default() ) }
    ),
    sequencer::TrackDefinition::new(
      TrackInfo{ id: TrackId::Windmill, name: String::from( "Windmill" ) },
      ||{ Box::new( windmill::Windmill::new() ) })
  ]
});
