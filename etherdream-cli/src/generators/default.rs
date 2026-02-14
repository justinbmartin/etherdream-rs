use std::f64::consts::TAU;

use nalgebra as na;

use crate::page::Page;

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

pub fn generate( buf: &mut [na::Point3<f64>], page: Page, queue_rate: usize, scale: f64, rotations_per_second: f64 ) -> Page {
  let mut buf_index: usize = 0;
  let mut frame: usize = 0;
  let mut matrix: na::Matrix4<f64>;
  let mut rotation: na::Rotation3<f64>;
  let scale = na::Scale3::new( scale, scale, scale );
  let mut time: f64;

  // We start rendering at the page-defined offset for the first "frame". This
  // gets reset to `0` on every subsequent "frame".
  assert!( page.offset < SHAPE.len(), "page offset must be less than shape's length" );
  let mut point_index: usize = page.offset;

  loop {
    // Calculate the start time for this frame
    time = page.time + ( ( frame * SHAPE.len() ) as f64 / queue_rate as f64 );

    // Calculate our transformation matrix
    rotation = na::Rotation3::from_axis_angle( &na::Vector3::z_axis(), time * rotations_per_second * -1.0 * TAU );
    matrix = rotation.to_homogeneous() * scale.to_homogeneous();

    for ( i, point ) in SHAPE[point_index..].iter().enumerate() {
      buf[buf_index] = matrix.transform_point( &point );
      buf_index += 1;

      if buf.len() == buf_index {
        return Page{ time, offset: i + 1 };
      }
    }

    // Reset `point_index` and increment our `frame`
    point_index = 0;
    frame += 1;
  }
}