mod device;
mod list;

// Export our scenes
pub use device::DeviceScene;
pub use list::ListScene;

#[derive( Clone, Copy, Eq, Hash, PartialEq )]
pub enum SceneKey { Device, List }