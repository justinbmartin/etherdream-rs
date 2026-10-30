use std::collections::BTreeMap;
use std::sync::LazyLock;

use crate::sequencer;

mod beam;
mod common;
mod windmill;

//
pub use beam::Beam;
pub use windmill::Windmill;

//
pub const BEAM_ID: &str = "beam";
pub const WINDMILL_ID: &str = "windmill";

pub struct TrackDefinition {
  pub create_fn: fn() -> Box<dyn sequencer::Track>,
  pub id: String,
  pub name: String
}

pub static TRACK_MAP: LazyLock<BTreeMap<&str, TrackDefinition>> = LazyLock::new(|| {
  let mut definitions: BTreeMap<&str, TrackDefinition> = BTreeMap::new();

  definitions.insert( BEAM_ID, TrackDefinition{
    create_fn: ||{ Box::new( beam::Beam::default() ) },
    id: String::from( BEAM_ID ),
    name: String::from( "Beam" )
  });

  definitions.insert( WINDMILL_ID, TrackDefinition{
    create_fn: ||{ Box::new( windmill::Windmill::new() ) },
    id: String::from( WINDMILL_ID ),
    name: String::from( "Windmill" )
  });

  definitions
});