use etherdream::generator;

use super::common;

#[derive( Default )]
pub(crate) struct Beam;

impl generator::Executable for Beam {
  fn on_start( &mut self, _: generator::OnStartContext ) -> generator::Config {
    generator::Config::new( 1_000 )
  }

  fn execute( &mut self, ctx: &mut generator::ExecutionContext ) {
    let color = common::ilda_color( 1.0 );
    let pos = common::ilda_position( 0.0 );

    for _ in 0..ctx.remaining() {
      ctx.push_point( pos, pos, color, color, color );
    }
  }
}