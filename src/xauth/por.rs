use async_std::task;
use libp2p::{
    core::{upgrade, ConnectedPoint},
    futures::{channel::mpsc, StreamExt},
    noise, ping,
    swarm::NetworkBehaviour,
    Multiaddr, PeerId, StreamProtocol, Swarm,
};
use std::{error::Error, time::Duration};

pub const PROTOCOL_NAME: StreamProtocol = StreamProtocol::new("/xauth/1.0.0");
use libp2p::identity::{Keypair, PublicKey};
#[derive(Debug, Clone)]
pub struct RepresentRecord {
    pub owner_public_key: PublicKey,
    pub peer_id: PeerId,
    pub signature: Vec<u8>,
}

impl RepresentRecord {
    pub fn new(owner_key_pair: &Keypair, peer_id: PeerId) -> Result<Self, Box<dyn Error>> {
        // Подписываем server_peer_id ключом организации
        let signature = owner_key_pair.sign(&peer_id.to_bytes())?;
        Ok(Self {
            owner_public_key: owner_key_pair.public(),
            peer_id: peer_id.clone(),
            signature: signature.to_vec(),
        })
    }

    pub fn check(&self) -> Result<PeerId, Box<dyn Error>> {
        if self
            .owner_public_key
            .verify(&self.peer_id.to_bytes(), &self.signature)
        {
            return Ok(self.owner_public_key.to_peer_id());
        } else {
            return Err("so bad(".into());
        }
    }
}
