#[derive( Debug )]
pub enum Action {
  Connect( u16 ),
  SelectDevice( usize ),
  DeselectDevice
}