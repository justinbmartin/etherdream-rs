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



pub const TRACKS: LazyLock<Vec<sequencer::TrackDefinition<TrackId>>> = LazyLock::new( ||{
  vec![
    sequencer::TrackDefinition::new( TrackId::Beam, ||{ Box::new( beam::Beam::default() ) }),
    sequencer::TrackDefinition::new( TrackId::Windmill, ||{ Box::new( windmill::Windmill::new() ) })
  ]
});
