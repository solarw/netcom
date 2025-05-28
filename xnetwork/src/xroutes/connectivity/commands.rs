use super::relay_client::commands::RelayClientCommand;
use super::relay_server::commands::RelayServerCommand;

#[derive(Debug)]
pub enum ConnectivityCommand {
    /// Relay client commands
    RelayClient(RelayClientCommand),
    /// Relay server commands
    RelayServer(RelayServerCommand),
}

impl From<RelayClientCommand> for ConnectivityCommand {
    fn from(command: RelayClientCommand) -> Self {
        ConnectivityCommand::RelayClient(command)
    }
}

impl From<RelayServerCommand> for ConnectivityCommand {
    fn from(command: RelayServerCommand) -> Self {
        ConnectivityCommand::RelayServer(command)
    }
}
