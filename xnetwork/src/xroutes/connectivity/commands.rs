use super::relay_server::commands::RelayServerCommand;

#[derive(Debug)]
pub enum ConnectivityCommand {
    /// Relay server commands
    RelayServer(RelayServerCommand),
}


impl From<RelayServerCommand> for ConnectivityCommand {
    fn from(command: RelayServerCommand) -> Self {
        ConnectivityCommand::RelayServer(command)
    }
}
