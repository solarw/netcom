// ./xroutes/discovery/commands.rs

use super::mdns::commands::MdnsCommand;
use super::kad::commands::KadCommand;

#[derive(Debug)]
pub enum DiscoveryCommand {
    /// mDNS discovery commands
    Mdns(MdnsCommand),
    
    /// Kademlia DHT commands
    Kad(KadCommand),
}