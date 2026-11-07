use std::fmt;

use crate::sequencer;

mod beam;
mod common;
mod windmill;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Track Identifiers

#[derive( Clone, Copy, Debug, Eq, Hash, PartialEq )]
pub enum Track {
  Beam,
  Windmill
}

impl sequencer::TrackList for Track {
  fn new( &self ) -> Box<dyn sequencer::Track> {
    match self {
      Self::Beam => Box::new( beam::Beam::default() ),
      Self::Windmill => Box::new( windmill::Windmill::new() )
    }
  }
}

impl fmt::Display for Track {
  fn fmt( &self, f: &mut fmt::Formatter<'_> ) -> fmt::Result {
    match self {
      Self::Beam => write!( f, "Beam" ),
      Self::Windmill => write!( f, "Windmill" )
    }
  }
}