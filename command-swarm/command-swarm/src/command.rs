//! SwarmCommand trait for type-safe command execution with results

use std::error::Error;
use tokio::sync::oneshot;

/// Trait for swarm commands that can return typed results
/// 
/// Commands implementing this trait can be executed and return
/// type-safe results through oneshot channels
pub trait SwarmCommand: Send + 'static {
    /// The type of result returned by this command
    type Output: Send + 'static;
}
