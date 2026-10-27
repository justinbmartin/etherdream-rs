use std::collections::BTreeMap;
use std::sync::LazyLock;

mod beam;
mod common;
mod windmill;

use etherdream::generator::Executable;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Executor

pub struct Executor {
  create_fn: fn() -> Box<dyn Executable>,
  id: String,
  name: String
}

impl Executor {
  pub fn create( &self ) -> Box<dyn Executable> { ( self.create_fn )() }
  pub fn id( &self ) -> &str { &self.id }
  pub fn name( &self ) -> &str { &self.name }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Executors

pub const BEAM_ID: &str = "beam";
pub const WINDMILL_ID: &str = "windmill";

pub static EXECUTORS: LazyLock<BTreeMap<&str, Executor>> = LazyLock::new(|| {
  let mut executors: BTreeMap<&str, Executor> = BTreeMap::new();

  executors.insert( BEAM_ID, Executor{
    create_fn: ||{ Box::new( beam::Beam::default() ) },
    id: String::from( BEAM_ID ),
    name: String::from( "Beam" )
  });

  executors.insert( WINDMILL_ID, Executor{
    create_fn: ||{ Box::new( windmill::Windmill::new() ) },
    id: String::from( WINDMILL_ID ),
    name: String::from( "Windmill" )
  });

  executors
});