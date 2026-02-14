use etherdream_cli::generators::default;
use etherdream_cli::page::Page;
use nalgebra as na;

#[tokio::test]
async fn debug_test() {
  let mut page = Page::default();
  let queue_rate: usize = 1_000;
  let rots_per_sec: f64 = 0.5;
  let scale: f64 = 0.25;

  let mut buf1 = vec![na::Point3::<f64>::default(); 16];
  let next_page = default::generate( &mut buf1, page, queue_rate, scale, rots_per_sec );
  assert_eq!( next_page.offset, 4 );

  page.offset = 1;
  let mut buf2 = vec![na::Point3::<f64>::default(); 16];
  let _ = default::generate( &mut buf2, page, queue_rate, scale, rots_per_sec );

  assert_eq!( buf1[1], buf2[0] );
  assert_eq!( buf1[2], buf2[1] );
}