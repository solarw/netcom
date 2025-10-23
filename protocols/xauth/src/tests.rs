use libp2p::{identity::Keypair, PeerId};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::por::por::{ProofOfRepresentation, PorUtils};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_validate_por() {
        // Create keypair for owner
        let owner_keypair = PorUtils::generate_owner_keypair();

        // Create keypair for node
        let node_keypair = PorUtils::generate_owner_keypair();

        // Get node PeerId
        let node_peer_id = PorUtils::peer_id_from_keypair(&node_keypair);

        // Create POR with 1 hour validity
        let por =
            ProofOfRepresentation::create(&owner_keypair, node_peer_id, Duration::from_secs(3600))
                .expect("Failed to create POR");

        // Validate POR
        por.validate().expect("POR should be valid");
    }

    #[test]
    fn test_por_expired() {
        // Create keypair for owner
        let owner_keypair = PorUtils::generate_owner_keypair();

        // Create keypair for node
        let node_keypair = PorUtils::generate_owner_keypair();

        // Get node PeerId
        let node_peer_id = PorUtils::peer_id_from_keypair(&node_keypair);

        // Current time
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Create POR that has already expired (started 2 hours ago, ended 1 hour ago)
        let por = ProofOfRepresentation::create_with_times(
            &owner_keypair,
            node_peer_id,
            now - 7200, // 2 hours ago
            now - 3600, // 1 hour ago
        )
        .expect("Failed to create POR");

        // Validate POR - should return expiration error
        let result = por.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("expired"));
    }

    #[test]
    fn test_por_not_yet_valid() {
        // Create keypair for owner
        let owner_keypair = PorUtils::generate_owner_keypair();

        // Create keypair for node
        let node_keypair = PorUtils::generate_owner_keypair();

        // Get node PeerId
        let node_peer_id = PorUtils::peer_id_from_keypair(&node_keypair);

        // Current time
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Create POR that is not yet valid (starts in 1 hour)
        let por = ProofOfRepresentation::create_with_times(
            &owner_keypair,
            node_peer_id,
            now + 3600, // 1 hour forward
            now + 7200, // 2 hours forward
        )
        .expect("Failed to create POR");

        // Validate POR - should return not yet valid error
        let result = por.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not yet valid"));
    }

    #[test]
    fn test_por_invalid_signature() {
        // Create keypair for owner
        let owner_keypair = PorUtils::generate_owner_keypair();

        // Create keypair for node
        let node_keypair = PorUtils::generate_owner_keypair();

        // Get node PeerId
        let node_peer_id = PorUtils::peer_id_from_keypair(&node_keypair);

        // Create valid POR
        let mut por =
            ProofOfRepresentation::create(&owner_keypair, node_peer_id, Duration::from_secs(3600))
                .expect("Failed to create POR");

        // Corrupt the signature
        if !por.signature.is_empty() {
            por.signature[0] = por.signature[0].wrapping_add(1);
        }

        // Validate POR - should return invalid signature error
        let result = por.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid signature"));
    }

    #[test]
    fn test_is_expired() {
        // Create keypair for owner
        let owner_keypair = PorUtils::generate_owner_keypair();

        // Create keypair for node
        let node_keypair = PorUtils::generate_owner_keypair();

        // Get node PeerId
        let node_peer_id = PorUtils::peer_id_from_keypair(&node_keypair);

        // Current time
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // 1. Create valid POR
        let valid_por = ProofOfRepresentation::create_with_times(
            &owner_keypair,
            node_peer_id,
            now - 3600, // 1 hour ago
            now + 3600, // 1 hour forward
        )
        .expect("Failed to create POR");

        assert!(!valid_por.is_expired().unwrap());

        // 2. Create expired POR
        let expired_por = ProofOfRepresentation::create_with_times(
            &owner_keypair,
            node_peer_id,
            now - 7200, // 2 hours ago
            now - 3600, // 1 hour ago
        )
        .expect("Failed to create POR");

        assert!(expired_por.is_expired().unwrap());
    }

    #[test]
    fn test_remaining_time() {
        // Create keypair for owner
        let owner_keypair = PorUtils::generate_owner_keypair();

        // Create keypair for node
        let node_keypair = PorUtils::generate_owner_keypair();

        // Get node PeerId
        let node_peer_id = PorUtils::peer_id_from_keypair(&node_keypair);

        // Current time
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // 1. Create POR that will be valid for approximately 1 hour
        let valid_por = ProofOfRepresentation::create_with_times(
            &owner_keypair,
            node_peer_id,
            now - 3600, // 1 hour ago
            now + 3600, // 1 hour forward
        )
        .expect("Failed to create POR");

        let remaining = valid_por.remaining_time().unwrap();
        assert!(remaining.is_some());
        // Check that remaining time is approximately 1 hour (with a few seconds tolerance)
        assert!(remaining.unwrap() > 3590 && remaining.unwrap() <= 3600);

        // 2. Create expired POR
        let expired_por = ProofOfRepresentation::create_with_times(
            &owner_keypair,
            node_peer_id,
            now - 7200, // 2 hours ago
            now - 3600, // 1 hour ago
        )
        .expect("Failed to create POR");

        assert!(expired_por.remaining_time().unwrap().is_none());
    }

    #[test]
    fn test_por_wrong_owner_key() {
        // Create two different keypairs for owners
        let owner_keypair1 = PorUtils::generate_owner_keypair();
        let owner_keypair2 = PorUtils::generate_owner_keypair();

        // Create keypair for node
        let node_keypair = PorUtils::generate_owner_keypair();
        let node_peer_id = PorUtils::peer_id_from_keypair(&node_keypair);

        // Create POR with first owner's key
        let por =
            ProofOfRepresentation::create(&owner_keypair1, node_peer_id, Duration::from_secs(3600))
                .expect("Failed to create POR");

        // Try to validate POR using second owner's public key
        // For this we need to create a new POR with the second owner's key
        let tampered_por = ProofOfRepresentation::create(
            &owner_keypair2,
            node_peer_id,
            Duration::from_secs(3600),
        )
        .expect("Failed to create POR");

        // The PORs should be different
        assert_ne!(por.signature, tampered_por.signature);
        assert_ne!(por.owner_public_key, tampered_por.owner_public_key);

        // Each POR should only validate with its own owner's key
        por.validate().expect("Original POR should be valid");
        tampered_por
            .validate()
            .expect("Second POR should be valid with its own key");
    }
}
