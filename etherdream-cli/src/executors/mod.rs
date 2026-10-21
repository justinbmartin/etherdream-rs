use std::collections::HashMap;
use std::sync::LazyLock;

mod windmill;

use etherdream::generator::Executable;

pub static EXECUTORS: LazyLock<HashMap<&str, fn() -> Box<dyn Executable>>> = LazyLock::new(|| {
  let mut executors: HashMap<&str, fn() -> Box<dyn Executable>> = HashMap::new();
  executors.insert( "windmill", ||{ Box::new( windmill::Windmill::new() ) } );
  executors
});