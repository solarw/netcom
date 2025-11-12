//! KeepAlive commands for XNetwork2

use tokio::sync::oneshot;

/// Commands for KeepAlive behaviour
#[derive(Debug)]
pub enum KeepAliveCommand {
    /// Set keep-alive status
    SetKeepAlive {
        enabled: bool,
        response: oneshot::Sender<Result<(), Box<dyn std::error::Error + Send + Sync>>>,
    },
    /// Get keep-alive status
    GetKeepAliveStatus {
        response: oneshot::Sender<Result<bool, Box<dyn std::error::Error + Send + Sync>>>,
    },
}
