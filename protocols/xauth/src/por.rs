use libp2p::{
    identity::{Keypair, PublicKey},
    PeerId,
};
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// Create a module with serialization/deserialization logic for PublicKey
mod public_key_serde {
    use libp2p::identity::PublicKey;
    use serde::{de, Deserializer, Serializer};
    use std::fmt;

    pub fn serialize<S>(key: &PublicKey, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Convert PublicKey to protobuf bytes
        let bytes = key.encode_protobuf();
        // Serialize the bytes
        serializer.serialize_bytes(&bytes)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<PublicKey, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Define a visitor that accepts bytes
        struct PublicKeyVisitor;

        impl<'de> de::Visitor<'de> for PublicKeyVisitor {
            type Value = PublicKey;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a byte array containing a protobuf-encoded PublicKey")
            }

            fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                PublicKey::try_decode_protobuf(value)
                    .map_err(|e| de::Error::custom(format!("Invalid public key: {}", e)))
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                // Collect bytes from sequence
                let mut bytes = Vec::new();
                while let Some(byte) = seq.next_element()? {
                    bytes.push(byte);
                }

                PublicKey::try_decode_protobuf(&bytes)
                    .map_err(|e| de::Error::custom(format!("Invalid public key: {}", e)))
            }
        }

        // Use the visitor to deserialize
        deserializer.deserialize_bytes(PublicKeyVisitor)
    }
}

/// Module for working with Proof of Representation
pub mod por {
    use super::*;

    /// Proof of Representation (POR) structure
    /// Represents a credential certifying that a node represents the owner
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ProofOfRepresentation {
        /// Owner's public key
        #[serde(with = "public_key_serde")]
        pub owner_public_key: PublicKey,

        /// PeerId of the node representing the owner
        pub peer_id: PeerId,

        /// Timestamp of credential creation (UNIX timestamp in seconds)
        pub issued_at: u64,

        /// Expiration time of the credential (UNIX timestamp in seconds)
        pub expires_at: u64,

        /// Digital signature created with the owner's private key
        pub signature: Vec<u8>,
    }

    impl ProofOfRepresentation {
        /// Create a new POR with a valid signature
        ///
        /// # Arguments
        /// * `owner_keypair` - Owner's Keypair for signing
        /// * `peer_id` - PeerId of the node being delegated authority
        /// * `validity_duration` - Validity period of the credential
        ///
        /// # Returns
        /// A new ProofOfRepresentation instance with a valid signature
        pub fn create(
            owner_keypair: &Keypair,
            peer_id: PeerId,
            validity_duration: Duration,
        ) -> Result<Self, String> {
            // Get current time
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| format!("System time error: {}", e))?
                .as_secs();

            // Calculate expiration time
            let expires_at = now + validity_duration.as_secs();

            // Get public key
            let public_key = owner_keypair.public();

            // Prepare data for signing
            let message = Self::prepare_message_for_signing(&public_key, &peer_id, now, expires_at);

            // Create signature
            let signature = owner_keypair
                .sign(&message)
                .map_err(|e| format!("Signature creation error: {}", e))?;

            Ok(Self {
                owner_public_key: public_key,
                peer_id,
                issued_at: now,
                expires_at,
                signature,
            })
        }

        /// Create POR with specified start and end times (for testing)
        pub fn create_with_times(
            owner_keypair: &Keypair,
            peer_id: PeerId,
            issued_at: u64,
            expires_at: u64,
        ) -> Result<Self, String> {
            // Get public key
            let public_key = owner_keypair.public();

            // Prepare data for signing
            let message =
                Self::prepare_message_for_signing(&public_key, &peer_id, issued_at, expires_at);

            // Create signature
            let signature = owner_keypair
                .sign(&message)
                .map_err(|e| format!("Signature creation error: {}", e))?;

            Ok(Self {
                owner_public_key: public_key,
                peer_id,
                issued_at,
                expires_at,
                signature,
            })
        }

        /// Validate the POR
        ///
        /// # Returns
        /// `Ok(())` if POR is valid, otherwise `Err` with a description of the issue
        pub fn validate(&self) -> Result<(), String> {
            // Check validity period
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| format!("System time error: {}", e))?
                .as_secs();

            if now < self.issued_at {
                return Err("POR not yet valid".to_string());
            }

            if now > self.expires_at {
                return Err("POR has expired".to_string());
            }

            // Verify signature
            self.verify_signature()
        }

        /// Verify POR signature
        ///
        /// # Returns
        /// `Ok(())` if signature is valid, otherwise `Err` with description of the issue
        fn verify_signature(&self) -> Result<(), String> {
            // Prepare data for signature verification
            let message = Self::prepare_message_for_signing(
                &self.owner_public_key,
                &self.peer_id,
                self.issued_at,
                self.expires_at,
            );

            // Verify signature
            if self.owner_public_key.verify(&message, &self.signature) {
                Ok(())
            } else {
                Err("Invalid signature".to_string())
            }
        }

        /// Prepare message for signing/verification
        fn prepare_message_for_signing(
            owner_public_key: &PublicKey,
            peer_id: &PeerId,
            issued_at: u64,
            expires_at: u64,
        ) -> Vec<u8> {
            // Concatenate all data to be signed
            let mut message = Vec::new();

            // Add owner's public key bytes
            let public_key_bytes = owner_public_key.encode_protobuf();
            message.extend_from_slice(&public_key_bytes);

            // Add peer_id as string
            let peer_id_str = peer_id.to_string();
            message.extend_from_slice(peer_id_str.as_bytes());

            // Add issued_at and expires_at as bytes
            message.extend_from_slice(&issued_at.to_le_bytes());
            message.extend_from_slice(&expires_at.to_le_bytes());

            message
        }

        /// Check if POR has expired
        pub fn is_expired(&self) -> Result<bool, String> {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| format!("System time error: {}", e))?
                .as_secs();

            Ok(now > self.expires_at)
        }

        /// Get remaining validity time in seconds
        pub fn remaining_time(&self) -> Result<Option<u64>, String> {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| format!("System time error: {}", e))?
                .as_secs();

            if now > self.expires_at {
                Ok(None)
            } else {
                Ok(Some(self.expires_at - now))
            }
        }
    }

    /// Helper functions for working with keys
    pub struct PorUtils;

    impl PorUtils {
        /// Create a new keypair for the owner
        pub fn generate_owner_keypair() -> Keypair {
            Keypair::generate_ed25519()
        }

        /// Create a keypair from existing private key in protobuf format
        pub fn keypair_from_bytes(secret_key_bytes: &[u8]) -> Result<Keypair, String> {
            Keypair::from_protobuf_encoding(secret_key_bytes)
                .map_err(|e| format!("Invalid key format: {}", e))
        }

        /// Get PeerId from keypair
        pub fn peer_id_from_keypair(keypair: &Keypair) -> PeerId {
            keypair.public().to_peer_id()
        }
    }
}
