#[derive( Clone, Copy, Debug )]
pub struct Page {
  pub time: f64,
  pub offset: usize
}

impl Default for Page {
  fn default() -> Self {
    Self{ time: 0.0, offset: 0 }
  }
}