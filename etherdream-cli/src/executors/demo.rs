use std::f64::consts::TAU;

use nalgebra as na;

use etherdream::generator;

const QUEUE_RATE: usize = 1_000;
const ROTATIONS_PER_SECOND: f64 = 0.25;
const SCALE: na::Scale3<f64> = na::Scale3::<f64>::new( 0.2, 0.2, 0.2 );
const W: f64 = 0.25; // width

const SHAPE: [na::Point3<f64>; 12] = [
  // left arm (starting from top-left, counter clock-wise)
  na::point!( -1.0,      0.0 + W,  0.0 ),
  na::point!( -1.0,      0.0 - W,  0.0 ),
  na::point!(  0.0 - W,  0.0 - W,  0.0 ),

  // bottom arm
  na::point!(  0.0 - W, -1.0,      0.0 ),
  na::point!(  0.0 + W, -1.0,      0.0 ),
  na::point!(  0.0 + W,  0.0 - W,  0.0 ),

  // right arm
  na::point!(  1.0,      0.0 - W,  0.0 ),
  na::point!(  1.0,      0.0 + W,  0.0 ),
  na::point!(  0.0 + W,  0.0 + W,  0.0 ),

  // top arm
  na::point!(  0.0 + W,  1.0,      0.0 ),
  na::point!(  0.0 - W,  1.0,      0.0 ),
  na::point!(  0.0 - W,  0.0 + W,  0.0 )
];

pub(crate) struct Demo {
  // The last point, via index, that was published for the shape, persisted
  // across `execute` invocations.
  offset: usize,
  // The time that the animation is rendering point-data for, persisted across
  // `execute` invocations.
  time: f64
}

impl Demo {
  pub(crate) fn new() -> Self { Self{ offset: 0, time: 0.0 } }
}

impl generator::Executable for Demo {
  fn on_start( &mut self, _: generator::OnStartContext ) -> generator::Config {
    generator::Config::new( 1_000 )
  }

  fn execute( &mut self, ctx: &mut generator::ExecutionContext ) {
    let axis = na::Unit::new_normalize( na::Vector3::new( 0.75, 0.75, 0.1 ) );
    let mut m: na::Matrix4<f64>;
    let mut p: na::Point3<f64>;
    let mut r: na::Rotation3<f64>;

    loop {
      // Calculate the transformation matrix for `time` and transform all
      // point's in shape.
      r = na::Rotation3::from_axis_angle( &axis, self.time * ROTATIONS_PER_SECOND * -1.0 * TAU );
      m = r.to_homogeneous() * SCALE.to_homogeneous();

      for ( i, shape_point ) in SHAPE[self.offset..].iter().enumerate() {
        p = m.transform_point( &shape_point );

        // Map z into [0.0, 1.0]
        let mapped_z = ( p.z + 1.0 ) * 0.5;

        ctx.push_point(
          ilda_position( p.x ),
          ilda_position( p.y ),
          ilda_color( 1.0 - mapped_z ),
          0,
          ilda_color( mapped_z )
        );

        // Break out of execution if there is no longer any point-data capacity
        if ctx.remaining() == 0 {
          self.offset = ( i + 1 ) % SHAPE.len();
          return;
        }
      }

      // Reset `offset` to zero and increment `time`
      self.offset = 0;
      self.time += SHAPE.len() as f64 / QUEUE_RATE as f64;
    }
  }
}

#[inline(always)]
fn ilda_position( amount: f64 ) -> i16 {
  ( na::clamp( amount, -1.0, 1.0 ) * i16::MAX as f64 ).round() as i16
}

#[inline(always)]
fn ilda_color( amount: f64 ) -> u16 {
  ( na::clamp( amount, 0.0, 1.0 ) * u16::MAX as f64 ).round() as u16
}