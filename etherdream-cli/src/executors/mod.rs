use std::collections::BTreeMap;
use std::sync::LazyLock;

mod beam;
mod common;
mod windmill;

use etherdream::generator::Executable;

pub const BEAM_ID: &str = "beam";
pub const WINDMILL_ID: &str = "windmill";

pub struct Executor {
  pub create_fn: fn() -> Box<dyn Executable>,
  pub name: String
}

pub static EXECUTORS: LazyLock<BTreeMap<&'static str, Executor>> = LazyLock::new(|| {
  let mut executors: BTreeMap<&str, Executor> = BTreeMap::new();

  executors.insert( BEAM_ID, Executor{
    create_fn: ||{ Box::new( beam::Beam::default() ) },
    name: String::from( "Beam" )
  });

  executors.insert( WINDMILL_ID, Executor{
    create_fn: ||{ Box::new( windmill::Windmill::new() ) },
    name: String::from( "Windmill" )
  });

  executors
});