use crossterm::event::KeyCode;
use ratatui::layout::{ Constraint, Layout };
use ratatui::widgets::{ Paragraph, Widget };

use crate::scene;
use crate::scenes;
use crate::state::{ self, State };

pub fn scene_info( builder: &mut scene::Builder<State> ) {
  {
    let device_map = state::ReadOnlyDeviceMap::new( builder.state().device_map.clone() );
    builder.add_scene( scenes::list::ID, Box::new( scenes::list::ListScene::new( device_map ) ) );
  }

  {
    let scoped_device = state::ScopedDevice::new( builder.state().device_id.clone(), builder.state().device_map.clone() );
    builder.add_scene( scenes::device::ID, Box::new( scenes::device::DeviceScene::new( scoped_device ) ) );
  }

  {
    let scoped_device = state::ScopedDevice::new( builder.state().device_id.clone(), builder.state().device_map.clone() );
    builder.add_scene( scenes::connect::ID, Box::new( scenes::connect::ConnectScene::new( scoped_device ) ) );
  }
}