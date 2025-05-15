use libp2p::identity;

pub fn make_new_key() -> identity::Keypair {
    identity::Keypair::generate_ed25519()
}
