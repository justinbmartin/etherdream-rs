use std::collections::BTreeMap;
use std::sync::LazyLock;

mod beam;
mod common;
mod windmill;

use etherdream::generator::Executable;

pub const BEAM_ID: &str = "beam";
pub const WINDMILL_ID: &str = "windmill";

pub struct Generator {
  pub create_executor_fn: fn() -> Box<dyn Executable>,
  pub name: String
}

pub static GENERATORS: LazyLock<BTreeMap<&str, Generator>> = LazyLock::new(|| {
  let mut generators: BTreeMap<&str, Generator> = BTreeMap::new();

  generators.insert( BEAM_ID, Generator{
    create_executor_fn: ||{ Box::new( beam::Beam::default() ) },
    name: String::from( "Beam" )
  });

  generators.insert( WINDMILL_ID, Generator{
    create_executor_fn: ||{ Box::new( windmill::Windmill::new() ) },
    name: String::from( "Windmill" )
  });

  generators
});