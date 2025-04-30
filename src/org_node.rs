
use libp2p::{
    core::{
        PeerId, 
        identity::{Keypair, PublicKey},
    },
    identity,
};
use ed25519_dalek::{
    Signature, 
    SigningKey, 
    VerifyingKey, 
    Signature as DalekSignature
};
use std::error::Error;

/// Структура для представления организации
#[derive(Clone)]
struct OrganizationIdentity {
    /// Peer ID организации
    org_peer_id: PeerId,
    
    /// Приватный ключ организации
    org_private_key: Keypair,
}

/// Структура для удостоверения узла организации
#[derive(Clone)]
struct NodeAuthorization {
    /// Peer ID узла
    node_peer_id: PeerId,
    
    /// Цифровая подпись организации
    org_signature: Vec<u8>,
}

impl OrganizationIdentity {
    /// Создание нового удостоверения организации
    fn new() -> Self {
        let org_keypair = identity::Keypair::generate_ed25519();
        let org_peer_id = PeerId::from(org_keypair.public());
        
        Self {
            org_peer_id,
            org_private_key: org_keypair,
        }
    }
    
    /// Авторизация узла от имени организации
    fn authorize_node(&self, node_peer_id: PeerId) -> Result<NodeAuthorization, Box<dyn Error>> {
        // Преобразуем ключ libp2p в ключ ed25519-dalek
        let signing_key = SigningKey::from_bytes(
            &self.org_private_key.try_into_ed25519()?.to_bytes()
        );
        
        // Создаем подпись peer id узла
        let signature = signing_key.sign(node_peer_id.to_bytes().as_slice());
        
        Ok(NodeAuthorization {
            node_peer_id,
            org_signature: signature.to_bytes().to_vec(),
        })
    }
}

impl NodeAuthorization {
    /// Проверка авторизации узла
    fn verify(&self, org_public_key: &PublicKey) -> Result<bool, Box<dyn Error>> {
        // Преобразуем публичный ключ организации
        let verifying_key = VerifyingKey::from_bytes(
            &org_public_key.try_into_ed25519()?.to_bytes()
        )?;
        
        // Создаем объект сигнатуры
        let signature = DalekSignature::from_bytes(&self.org_signature)?;
        
        // Проверяем подпись
        match verifying_key.verify(self.node_peer_id.to_bytes().as_slice(), &signature) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false)
        }
    }
}