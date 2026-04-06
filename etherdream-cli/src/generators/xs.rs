use std::f64::consts::TAU;

use nalgebra as na;

use etherdream::generator::{ Executable, ExecutionContext };

const QUEUE_RATE: usize = 1_000;
const ROTATIONS_PER_SECOND: f64 = 0.25;
const SCALE: na::Scale3<f64> = na::Scale3::<f64>::new( 0.2, 0.2, 0.2 );
const W: f64 = 0.25; // width

const SHAPE: [na::Point3<f64>; 12] = [
  // left arm (starting from top-left, counter clock-wise)
  na::Point3::new( -1.0,      0.0 + W,  0.0 ),
  na::Point3::new( -1.0,      0.0 - W,  0.0 ),
  na::Point3::new(  0.0 - W,  0.0 - W,  0.0 ),

  // bottom arm
  na::Point3::new(  0.0 - W, -1.0,      0.0 ),
  na::Point3::new(  0.0 + W, -1.0,      0.0 ),
  na::Point3::new(  0.0 + W,  0.0 - W,  0.0 ),

  // right arm
  na::Point3::new(  1.0,      0.0 - W,  0.0 ),
  na::Point3::new(  1.0,      0.0 + W,  0.0 ),
  na::Point3::new(  0.0 + W,  0.0 + W,  0.0 ),

  // top arm
  na::Point3::new(  0.0 + W,  1.0,      0.0 ),
  na::Point3::new(  0.0 - W,  1.0,      0.0 ),
  na::Point3::new(  0.0 - W,  0.0 + W,  0.0 )
];

const FRAME_DURATION: f64 = SHAPE.len() as f64 / QUEUE_RATE as f64;

pub(crate) struct Xs {
  offset: usize,
  time: f64,
}

impl Xs {
  pub fn new() -> Self {
    Self{
      offset: 0,
      time: 0.0
    }
  }
}

impl Executable for Xs {
  fn execute( &mut self, ctx: &mut ExecutionContext ) {
    let axis = na::Unit::new_normalize( na::Vector3::new( 0.75, 0.75, 0.1 ) );
    let mut m: na::Matrix4<f64>;
    let mut p: na::Point3<f64>;
    let mut r: na::Rotation3<f64>;

    // We start rendering at the page-defined offset for the first "frame". This
    // gets reset to `0` on every subsequent "frame".
    assert!( self.offset < SHAPE.len(), "page offset must be less than shape's length" );

    loop {
      // Calculate the transformation matrix for `time` and transform all
      // point's in shape.
      r = na::Rotation3::from_axis_angle(&axis, self.time * ROTATIONS_PER_SECOND * -1.0 * TAU );
      m = r.to_homogeneous() * SCALE.to_homogeneous();

      for ( i, shape_point ) in SHAPE[self.offset..].iter().enumerate() {
        p = m.transform_point( &shape_point );

        ctx.push_point(
          mix_f64_i16( -1.0, 1.0, p.x ),
          mix_f64_i16( -1.0, 1.0, p.y ),
          mix_f64_i16( -1.0, 1.0, p.z * -1.0 ) as u16,
          0,
          mix_f64_i16( -1.0, 1.0, p.z ) as u16
        );

        //println!("z={:?}", p.z );

        // Break out of generator if we have published all available points.
        if ctx.remaining() == 0 {
          self.offset = ( i + 1 ) % SHAPE.len();
          return;
        }
      }

      // Reset `offset` to zero and increment `time`
      self.offset = 0;
      self.time += FRAME_DURATION;
    }
  }
}

#[inline(always)]
fn mix_f64_i16( min: f64, max: f64, amount: f64) -> i16 {
  ( na::clamp( amount, min, max ) * i16::MAX as f64 ).round() as i16
}