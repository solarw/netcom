use super::relay_server::events::RelayServerEvent;

#[derive(Debug, Clone)]
pub enum ConnectivityEvent {
    /// Relay server events
    RelayServer(RelayServerEvent),
}


impl From<RelayServerEvent> for ConnectivityEvent {
    fn from(event: RelayServerEvent) -> Self {
        ConnectivityEvent::RelayServer(event)
    }
}
