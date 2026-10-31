use std::sync::LazyLock;

use crate::device::Track;

mod beam;
mod common;
mod windmill;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Track Identifiers

pub const BEAM_ID: &str = "beam";
pub const WINDMILL_ID: &str = "windmill";

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Track Definition

pub struct TrackDefinition {
  pub create_fn: fn() -> Box<dyn Track>,
  pub id: String,
  pub name: String
}

pub static DEFINITIONS: LazyLock<Vec<TrackDefinition>> = LazyLock::new(|| {
  let mut definitions: Vec<TrackDefinition> = Vec::new();

  definitions.push( TrackDefinition{
    create_fn: ||{ Box::new( beam::Beam::default() ) },
    id: String::from( BEAM_ID ),
    name: String::from( "Beam" )
  });

  definitions.push( TrackDefinition{
    create_fn: ||{ Box::new( windmill::Windmill::new() ) },
    id: String::from( WINDMILL_ID ),
    name: String::from( "Windmill" )
  });

  definitions
});