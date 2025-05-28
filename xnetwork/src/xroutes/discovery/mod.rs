pub mod behaviour;
pub mod commands;
pub mod events;
pub mod mdns;
pub mod kad;

// Re-export main components
pub use behaviour::DiscoveryBehaviour;
pub use commands::DiscoveryCommand;
pub use events::{DiscoveryEvent, DiscoverySource};
