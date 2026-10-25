use std::collections::HashMap;
use std::sync::LazyLock;

mod windmill;

use etherdream::generator::Executable;

pub const WINDMILL_ID: &str = "windmill";

pub struct Executor {
  pub create_fn: fn() -> Box<dyn Executable>,
  pub name: String
}

pub static EXECUTORS: LazyLock<HashMap<&'static str, Executor>> = LazyLock::new(|| {
  let mut executors: HashMap<&str, Executor> = HashMap::new();

  executors.insert( WINDMILL_ID, Executor{
    create_fn: ||{ Box::new( windmill::Windmill::new() ) },
    name: String::from( "Windmill" )
  });

  executors
});