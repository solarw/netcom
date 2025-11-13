//! XRoutes behaviour handler for XNetwork2
//!
//! Composite behaviour with toggle components for identify, mdns, and kad discovery.

mod behaviour;
mod command;
mod handler;
mod pending_task_manager;
mod types;

pub use behaviour::{XRoutesBehaviour, XRoutesBehaviourEvent};
pub use command::{XRoutesCommand, MdnsCacheStatus};
pub use handler::XRoutesHandler;
pub use pending_task_manager::PendingTaskManager;
pub use types::{XRoutesConfig, XRoutesStatus};
