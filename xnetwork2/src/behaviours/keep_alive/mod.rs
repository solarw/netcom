//! KeepAlive behaviour for XNetwork2
//!
//! Simple behaviour that controls connection keep-alive status.

pub mod behaviour;
pub mod command;
pub mod handler;
pub mod handler_impl;

// Re-export for convenience
pub use behaviour::KeepAliveBehaviour;
pub use command::KeepAliveCommand;
pub use handler::KeepAliveConnectionHandler;
pub use handler_impl::KeepAliveHandler;
