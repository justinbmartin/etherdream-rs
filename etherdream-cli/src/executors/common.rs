use nalgebra as na;

#[inline(always)]
pub fn ilda_position( amount: f64 ) -> i16 {
  ( na::clamp( amount, -1.0, 1.0 ) * i16::MAX as f64 ).round() as i16
}

#[inline(always)]
pub fn ilda_color( amount: f64 ) -> u16 {
  ( na::clamp( amount, 0.0, 1.0 ) * u16::MAX as f64 ).round() as u16
}