use super::super::events::DiscoveryEvent;
use super::super::events::DiscoverySource;
use super::commands::MdnsCommand;
use libp2p::{
    Multiaddr, PeerId, identity, mdns,
    swarm::{
        NetworkBehaviour, THandlerInEvent, ToSwarm, behaviour::toggle::Toggle, derive_prelude::*,
    },
};
use std::collections::HashMap;
use std::collections::VecDeque;
use std::task::{Context, Poll};
pub struct MdnsBehaviour {
    pub mdns: Toggle<mdns::tokio::Behaviour>,
    pending_events: VecDeque<DiscoveryEvent>, // Буфер для событий
    key: identity::Keypair,
}

use super::events::MdnsEvent;

impl MdnsBehaviour {
    pub fn new(
        key: &identity::Keypair,
        enable_mdns: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mdns = if enable_mdns {
            match mdns::tokio::Behaviour::new(mdns::Config::default(), key.public().to_peer_id()) {
                Ok(mdns_behaviour) => Toggle::from(Some(mdns_behaviour)),
                Err(e) => {
                    tracing::warn!("Failed to create mDNS behavior: {}", e);
                    Toggle::from(None)
                }
            }
        } else {
            Toggle::from(None)
        };

        Ok(MdnsBehaviour {
            mdns,
            pending_events: VecDeque::new(),
            key: key.clone(),
        })
    }

    // ваши остальные методы...
    pub fn enable_mdns(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let key = self.key.clone();
        if self.mdns.is_enabled() {
            return Ok(());
        }

        match mdns::tokio::Behaviour::new(mdns::Config::default(), key.public().to_peer_id()) {
            Ok(mdns_behaviour) => {
                self.mdns = Toggle::from(Some(mdns_behaviour));
                Ok(())
            }
            Err(e) => {
                tracing::warn!("Failed to enable mDNS behavior: {}", e);
                Err(Box::new(e))
            }
        }
    }

    pub fn disable_mdns(&mut self) {
        self.mdns = Toggle::from(None);
    }

    pub fn is_mdns_enabled(&self) -> bool {
        self.mdns.is_enabled()
    }
}

impl MdnsBehaviour {
    pub fn handle_command(&mut self, cmd: MdnsCommand) {
        match cmd {
            MdnsCommand::EnableMdns => {
                self.enable_mdns();
            }

            MdnsCommand::DisableMdns => {
                self.disable_mdns();
            }
        }
    }
}

impl NetworkBehaviour for MdnsBehaviour {
    type ConnectionHandler =
        <Toggle<mdns::tokio::Behaviour> as NetworkBehaviour>::ConnectionHandler;
    type ToSwarm = DiscoveryEvent; // Ваш кастомный тип событий

    fn handle_established_inbound_connection(
        &mut self,
        connection_id: ConnectionId,
        peer: PeerId,
        local_addr: &Multiaddr,
        remote_addr: &Multiaddr,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        self.mdns.handle_established_inbound_connection(
            connection_id,
            peer,
            local_addr,
            remote_addr,
        )
    }

    fn handle_established_outbound_connection(
        &mut self,
        connection_id: ConnectionId,
        peer: PeerId,
        addr: &Multiaddr,
        role_override: Endpoint,
        port_use: PortUse,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        self.mdns.handle_established_outbound_connection(
            connection_id,
            peer,
            addr,
            role_override,
            port_use,
        )
    }

    fn on_swarm_event(&mut self, event: FromSwarm) {
        self.mdns.on_swarm_event(event)
    }

    fn on_connection_handler_event(
        &mut self,
        peer_id: PeerId,
        connection_id: ConnectionId,
        event: THandlerOutEvent<Self>,
    ) {
        self.mdns
            .on_connection_handler_event(peer_id, connection_id, event)
    }

    fn poll(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<ToSwarm<Self::ToSwarm, THandlerInEvent<Self>>> {
        if let Some(event) = self.pending_events.pop_front() {
            return Poll::Ready(ToSwarm::GenerateEvent(event));
        }
        // Получаем события от mdns
        match self.mdns.poll(cx) {
            Poll::Ready(ToSwarm::GenerateEvent(mdns_event)) => {
                // Фильтруем и преобразуем события
                match mdns_event {
                    mdns::Event::Discovered(peers) => {
                        let mut peer_groups: HashMap<PeerId, Vec<Multiaddr>> = HashMap::new();
                        for (peer_id, addr) in peers {
                            peer_groups
                                .entry(peer_id)
                                .or_insert_with(Vec::new)
                                .push(addr);
                        }

                        // Generate discovery events
                        for (peer_id, addresses) in peer_groups {
                            self.pending_events
                                .push_back(DiscoveryEvent::PeerDiscovered {
                                    peer_id,
                                    addresses,
                                    source: DiscoverySource::Mdns,
                                });
                        }
                    }
                    mdns::Event::Expired(peers) => {
                        for (peer_id, _addr) in peers {
                            self.pending_events.push_back(DiscoveryEvent::PeerExpired {
                                peer_id,
                                source: DiscoverySource::Mdns,
                            });
                        }
                    }
                    // Другие события mdns можно игнорировать или обрабатывать
                    _ => {
                        tracing::info!("Ignoring mdns event: {:?}", mdns_event);
                    }
                }
            }
            Poll::Ready(other_event) => {
                // Пропускаем неизмененными все остальные типы событий
                return Poll::Ready(other_event.map_out(|_| unreachable!()));
            }
            Poll::Pending => {}
        }
        if let Some(event) = self.pending_events.pop_front() {
            return Poll::Ready(ToSwarm::GenerateEvent(event));
        }

        Poll::Pending
    }
}
