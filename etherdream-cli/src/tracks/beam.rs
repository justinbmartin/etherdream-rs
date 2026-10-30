use etherdream::generator;

use super::common;

#[derive( Default )]
pub struct Beam;

impl super::Track for Beam {
  fn id( &self ) -> &str { "beam" }
  fn name( &self ) -> &str { "Beam" }

  fn execute( &mut self, ctx: &mut generator::ExecutionContext ) {
    let color = common::ilda_color( 1.0 );
    let pos = common::ilda_position( 0.0 );

    for _ in 0..ctx.remaining() {
      ctx.push_point( pos, pos, color, color, color );
    }
  }
}