use etherdream::generator::{ Executable, ExecutionContext };

pub(crate) struct Noop {}

impl Executable for Noop {
  fn execute( &mut self, _ctx: &mut ExecutionContext ) { }
}

impl Noop {
  pub(crate) fn new() -> Self {
    Self{}
  }
}