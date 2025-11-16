use std::collections::HashMap;
use std::time::Duration;
use std::time::Instant;

use super::definitions::AuthDirection;
use super::definitions::CombinedAuthState;
use super::definitions::DirectionalAuthState;
use libp2p::{swarm::ConnectionId, Multiaddr, PeerId};
// Improved connection data structure
pub struct ConnectionData {
    // Connection metadata
    pub peer_id: PeerId,
    pub connection_id: ConnectionId,
    pub address: Multiaddr,
    // When the connection was established
    pub established: Instant,
    // Last activity timestamp
    pub last_activity: Instant,
    // Inbound authentication state (remote peer authenticating us)
    pub inbound_auth: DirectionalAuthState,
    // Outbound authentication state (us authenticating remote peer)
    pub outbound_auth: DirectionalAuthState,
    // Timeout flags to make timeout events idempotent
    pub outbound_timed_out: bool,
    pub inbound_timed_out: bool,
}

impl ConnectionData {
    // Create a new connection data
    pub fn new(peer_id: PeerId, connection_id: ConnectionId, address: Multiaddr) -> Self {
        let now = Instant::now();
        Self {
            peer_id,
            connection_id,
            address,
            established: now,
            last_activity: now,
            inbound_auth: DirectionalAuthState::NotStarted,
            outbound_auth: DirectionalAuthState::NotStarted,
            outbound_timed_out: false,
            inbound_timed_out: false,
        }
    }

    // Update activity timestamp
    pub fn touch(&mut self) {
        self.last_activity = Instant::now();
    }

    // Start outbound authentication
    pub fn start_outbound_auth(&mut self) {
        self.outbound_auth = DirectionalAuthState::InProgress {
            started: Instant::now(),
        };
        self.touch();
    }

    // Start inbound authentication
    pub fn start_inbound_auth(&mut self) {
        self.inbound_auth = DirectionalAuthState::InProgress {
            started: Instant::now(),
        };
        self.touch();
    }

    // Set outbound auth as successful
    pub fn set_outbound_auth_success(&mut self, metadata: HashMap<String, String>) {
        self.outbound_auth = DirectionalAuthState::Successful(metadata);
        self.touch();
    }

    // Set inbound auth as successful
    pub fn set_inbound_auth_success(&mut self) {
        self.inbound_auth = DirectionalAuthState::Successful(HashMap::new());
        self.touch();
    }

    // Set outbound auth as failed
    pub fn set_outbound_auth_failed(&mut self, reason: String) {
        self.outbound_auth = DirectionalAuthState::Failed(reason);
        self.touch();
    }

    // Set inbound auth as failed
    pub fn set_inbound_auth_failed(&mut self, reason: String) {
        self.inbound_auth = DirectionalAuthState::Failed(reason);
        self.touch();
    }

    // Get the current combined auth state
    pub fn get_combined_state(&self) -> CombinedAuthState {
        match (&self.inbound_auth, &self.outbound_auth) {
            // If either direction has failed, the combined state is Failed
            (DirectionalAuthState::Failed(reason), _) => CombinedAuthState::Failed(reason.clone()),
            (_, DirectionalAuthState::Failed(reason)) => CombinedAuthState::Failed(reason.clone()),

            // If both directions are successful, the connection is fully authenticated
            (DirectionalAuthState::Successful(_), DirectionalAuthState::Successful(metadata)) => {
                CombinedAuthState::FullyAuthenticated(metadata.clone())
            }

            // If only one direction is authenticated
            (DirectionalAuthState::Successful(_), _) => CombinedAuthState::InboundOnly,
            (_, DirectionalAuthState::Successful(metadata)) => {
                CombinedAuthState::OutboundOnly(metadata.clone())
            }

            // Otherwise, not authenticated
            _ => CombinedAuthState::NotAuthenticated,
        }
    }

    // Check for authentication timeouts
    pub fn check_timeout(&self, auth_timeout: Duration) -> Option<AuthDirection> {
        let now = Instant::now();

        // Ignore timeout if both directions are NotStarted
        if matches!(self.inbound_auth, DirectionalAuthState::NotStarted)
            && matches!(self.outbound_auth, DirectionalAuthState::NotStarted)
        {
            return None;
        }

        // Check each authentication direction for timeout
        let inbound_timeout = match &self.inbound_auth {
            DirectionalAuthState::InProgress { started }
                if now.duration_since(*started) > auth_timeout =>
            {
                true
            }
            _ => false,
        };

        let outbound_timeout = match &self.outbound_auth {
            DirectionalAuthState::InProgress { started }
                if now.duration_since(*started) > auth_timeout =>
            {
                true
            }
            _ => false,
        };

        // Check for overall connection inactivity
        let conn_timeout = now.duration_since(self.last_activity) > auth_timeout * 1;

        // Return appropriate timeout direction

        if (self.is_some_failed()) {
            None
        } else if conn_timeout && !self.is_fully_authenticated() {
            Some(AuthDirection::Both)
        } else if inbound_timeout && outbound_timeout {
            Some(AuthDirection::Both)
        } else if inbound_timeout {
            Some(AuthDirection::Inbound)
        } else if outbound_timeout {
            Some(AuthDirection::Outbound)
        } else {
            None
        }
    }

    // Get peer's authentication metadata if available
    pub fn get_metadata(&self) -> Option<HashMap<String, String>> {
        match &self.outbound_auth {
            DirectionalAuthState::Successful(metadata) => Some(metadata.clone()),
            _ => None,
        }
    }

    // Check if peer is fully authenticated
    pub fn is_fully_authenticated(&self) -> bool {
        matches!(
            self.get_combined_state(),
            CombinedAuthState::FullyAuthenticated(_)
        )
    }

    pub fn is_some_failed(&self) -> bool {
        matches!(self.inbound_auth, DirectionalAuthState::Failed(_))
            || matches!(self.outbound_auth, DirectionalAuthState::Failed(_))
    }

    // Check if outbound auth is not started
    pub fn is_outbound_not_started(&self) -> bool {
        matches!(self.outbound_auth, DirectionalAuthState::NotStarted)
    }

    // Check if authentication is in progress in any direction
    pub fn is_authentication_in_progress(&self) -> bool {
        matches!(self.inbound_auth, DirectionalAuthState::InProgress { .. }) ||
        matches!(self.outbound_auth, DirectionalAuthState::InProgress { .. })
    }
}
