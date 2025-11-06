//! Behaviour handlers for XNetwork2
//!
//! Separate handlers for each protocol behaviour that implement
//! command-swarm's BehaviourHandler trait.

pub mod identify;
pub mod ping;
pub mod xauth;
pub mod xstream;

// Re-export handlers for convenience
pub use identify::IdentifyHandler;
pub use ping::PingHandler;
pub use xauth::XAuthHandler;
pub use xstream::XStreamHandler;

// Re-export command types
pub use identify::IdentifyCommand;
pub use ping::PingCommand;
pub use xauth::XAuthCommand;
pub use xstream::XStreamCommand;
