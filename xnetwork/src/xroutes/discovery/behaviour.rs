// XRoutes discovery behaviour combining mDNS and Kademlia

use libp2p::swarm::NetworkBehaviour;

use super::commands::DiscoveryCommand;
use super::mdns::behaviour::MdnsBehaviour;

#[derive(NetworkBehaviour)]
pub struct DiscoveryBehaviour {
    pub mdns: MdnsBehaviour,
}

impl DiscoveryBehaviour {
    fn handle_command(&mut self, cmd: DiscoveryCommand) {
        match cmd {
            DiscoveryCommand::Mdns(mdns_command) => self.mdns.handle_command(mdns_command),
        }
    }
}
