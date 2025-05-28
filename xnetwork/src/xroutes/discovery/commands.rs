use super::mdns::commands::MdnsCommand;

#[derive(Debug, Clone)]
pub enum DiscoveryCommand {
    Mdns(MdnsCommand),
}
