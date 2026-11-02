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

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Track Definition

pub struct TrackDefinition {
  pub create_fn: fn() -> Box<dyn sequencer::Track>,
  pub id: TrackId
}

pub static DEFINITIONS: LazyLock<Vec<TrackDefinition>> = LazyLock::new(|| {
  let mut definitions: Vec<TrackDefinition> = Vec::new();

  definitions.push( TrackDefinition{
    create_fn: ||{ Box::new( beam::Beam::default() ) },
    id: TrackId::Beam
  });

  definitions.push( TrackDefinition{
    create_fn: ||{ Box::new( windmill::Windmill::new() ) },
    id: TrackId::Windmill
  });

  definitions
});