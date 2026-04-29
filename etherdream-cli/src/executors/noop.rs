use etherdream::generator;

pub(crate) struct Noop {}

impl Noop {
  pub(crate) fn new() -> Self { Self{} }
}

impl generator::Executable for Noop {
  fn on_start( &mut self, _ : generator::OnStartContext ) -> generator::Config {
    generator::Config::new( 0 )
  }

  fn execute( &mut self, _ctx: &mut generator::ExecutionContext ) { }
}