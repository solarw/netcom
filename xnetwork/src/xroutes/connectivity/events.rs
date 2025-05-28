use super::relay_client::events::RelayClientEvent;
use super::relay_server::events::RelayServerEvent;

#[derive(Debug, Clone)]
pub enum ConnectivityEvent {
    /// Relay client events
    RelayClient(RelayClientEvent),
    /// Relay server events
    RelayServer(RelayServerEvent),
}

impl From<RelayClientEvent> for ConnectivityEvent {
    fn from(event: RelayClientEvent) -> Self {
        ConnectivityEvent::RelayClient(event)
    }
}

impl From<RelayServerEvent> for ConnectivityEvent {
    fn from(event: RelayServerEvent) -> Self {
        ConnectivityEvent::RelayServer(event)
    }
}
