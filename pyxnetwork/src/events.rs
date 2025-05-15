// src/py/events.rs
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::sync::Arc;

use libp2p::Multiaddr;
use libp2p::PeerId;

use xnetwork::events::NetworkEvent;
use xauth::events::PorAuthEvent;
use xauth::por::por::ProofOfRepresentation;
use crate::xstream::XStream as PyXStream;

use super::types::rust_por_to_py_por;
use super::types::ProofOfRepresentation as PyProofOfRepresentation;

/// Extract the numeric value from a ConnectionId
fn connection_id_to_u64(connection_id: &impl std::fmt::Debug) -> u64 {
    // Convert ConnectionId to a debug string
    let debug_str = format!("{:?}", connection_id);

    // Try to extract the numeric value
    // This assumes the debug format is something like "ConnectionId(42)"
    // and we want to extract the 42
    match debug_str.find('(') {
        Some(start_pos) => {
            match debug_str.find(')') {
                Some(end_pos) => {
                    if start_pos < end_pos {
                        let num_str = &debug_str[(start_pos + 1)..end_pos];
                        match num_str.parse::<u64>() {
                            Ok(id) => return id,
                            Err(_) => 0, // Default to 0 if parsing fails
                        }
                    } else {
                        0 // Malformed string
                    }
                }
                None => 0, // Missing closing parenthesis
            }
        }
        None => 0, // No opening parenthesis found
    }
}

/// Convert a NetworkEvent to a Python dictionary
pub fn network_event_to_dict(event: NetworkEvent) -> PyResult<PyObject> {
    Python::with_gil(|py| {
        let event_dict = PyDict::new(py);

        match event {
            NetworkEvent::PeerConnected { peer_id } => {
                event_dict.set_item("type", "PeerConnected")?;
                event_dict.set_item("peer_id", peer_id.to_string())?;
            }
            NetworkEvent::PeerDisconnected { peer_id } => {
                event_dict.set_item("type", "PeerDisconnected")?;
                event_dict.set_item("peer_id", peer_id.to_string())?;
            }
            NetworkEvent::ConnectionError { peer_id, error } => {
                event_dict.set_item("type", "ConnectionError")?;
                if let Some(pid) = peer_id {
                    event_dict.set_item("peer_id", pid.to_string())?;
                }
                event_dict.set_item("error", error)?;
            }
            NetworkEvent::ListeningOnAddress { addr, full_addr } => {
                event_dict.set_item("type", "ListeningOnAddress")?;
                event_dict.set_item("address", addr.to_string())?;
                if let Some(full) = full_addr {
                    event_dict.set_item("full_address", full.to_string())?;
                }
            }
            NetworkEvent::StopListeningOnAddress { addr } => {
                event_dict.set_item("type", "StopListeningOnAddress")?;
                event_dict.set_item("address", addr.to_string())?;
            }
            NetworkEvent::MdnsIsOn {} => {
                event_dict.set_item("type", "MdnsIsOn")?;
            }
            NetworkEvent::MdnsIsOff {} => {
                event_dict.set_item("type", "MdnsIsOff")?;
            }
            NetworkEvent::ConnectionOpened {
                peer_id,
                addr,
                connection_id,
                protocols,
            } => {
                event_dict.set_item("type", "ConnectionOpened")?;
                event_dict.set_item("peer_id", peer_id.to_string())?;
                event_dict.set_item("address", addr.to_string())?;
                // Convert connection_id to integer
                let conn_id_value = connection_id_to_u64(&connection_id);
                event_dict.set_item("connection_id", conn_id_value)?;
                let protocol_strings: Vec<String> =
                    protocols.iter().map(|p| p.to_string()).collect();
                event_dict.set_item("protocols", protocol_strings)?;
            }
            NetworkEvent::ConnectionClosed {
                peer_id,
                addr,
                connection_id,
            } => {
                event_dict.set_item("type", "ConnectionClosed")?;
                event_dict.set_item("peer_id", peer_id.to_string())?;
                event_dict.set_item("address", addr.to_string())?;
                // Convert connection_id to integer
                let conn_id_value = connection_id_to_u64(&connection_id);
                event_dict.set_item("connection_id", conn_id_value)?;
            }
            NetworkEvent::IncomingStream { stream } => {
                event_dict.set_item("type", "IncomingStream")?;

                // Create a PyXStream from the XStream
                let py_stream = PyXStream::from_arc_xstream(stream);
                // Create a Python object from the PyXStream
                let py_stream_obj = Py::new(py, py_stream)?;
                event_dict.set_item("stream", py_stream_obj)?;
            }
            NetworkEvent::KadAddressAdded { peer_id, addr } => {
                event_dict.set_item("type", "KadAddressAdded")?;
                event_dict.set_item("peer_id", peer_id.to_string())?;
                event_dict.set_item("address", addr.to_string())?;
            }
            NetworkEvent::KadRoutingUpdated { peer_id, addresses } => {
                event_dict.set_item("type", "KadRoutingUpdated")?;
                event_dict.set_item("peer_id", peer_id.to_string())?;
                // Convert addresses to strings
                let addr_strings: Vec<String> =
                    addresses.iter().map(|addr| addr.to_string()).collect();
                event_dict.set_item("addresses", addr_strings)?;
            }
            NetworkEvent::AuthEvent { event } => {
                event_dict.set_item("type", "AuthEvent")?;
                let auth_dict = auth_event_to_dict(py, &event)?;
                event_dict.set_item("auth_event", auth_dict)?;
            }
        }

        Ok(event_dict.to_object(py))
    })
}

/// Convert a PorAuthEvent to a Python dictionary
fn auth_event_to_dict(py: Python, event: &PorAuthEvent) -> PyResult<PyObject> {
    let auth_dict = PyDict::new(py);

    match event {
        PorAuthEvent::MutualAuthSuccess {
            peer_id,
            connection_id,
            address,
            metadata,
        } => {
            auth_dict.set_item("type", "MutualAuthSuccess")?;
            auth_dict.set_item("peer_id", peer_id.to_string())?;
            // Convert connection_id to integer
            let conn_id_value = connection_id_to_u64(connection_id);
            auth_dict.set_item("connection_id", conn_id_value)?;
            auth_dict.set_item("address", address.to_string())?;

            // Convert metadata HashMap to a Python dict
            let metadata_dict = PyDict::new(py);
            for (key, value) in metadata {
                metadata_dict.set_item(key, value)?;
            }
            auth_dict.set_item("metadata", metadata_dict)?;
        }
        PorAuthEvent::VerifyPorRequest {
            peer_id,
            connection_id,
            address,
            por,
            metadata,
        } => {
            auth_dict.set_item("type", "VerifyPorRequest")?;
            auth_dict.set_item("peer_id", peer_id.to_string())?;
            // Convert connection_id to integer
            let conn_id_value = connection_id_to_u64(connection_id);
            auth_dict.set_item("connection_id", conn_id_value)?;
            auth_dict.set_item("address", address.to_string())?;

            let py_por = rust_por_to_py_por(py, por.clone())?;
            auth_dict.set_item("por", py_por)?;

            // Convert metadata HashMap to a Python dict
            let metadata_dict = PyDict::new(py);
            for (key, value) in metadata {
                metadata_dict.set_item(key, value)?;
            }
            auth_dict.set_item("metadata", metadata_dict)?;
        }
        PorAuthEvent::OutboundAuthSuccess {
            peer_id,
            connection_id,
            address,
            metadata,
        } => {
            auth_dict.set_item("type", "OutboundAuthSuccess")?;
            auth_dict.set_item("peer_id", peer_id.to_string())?;
            // Convert connection_id to integer
            let conn_id_value = connection_id_to_u64(connection_id);
            auth_dict.set_item("connection_id", conn_id_value)?;
            auth_dict.set_item("address", address.to_string())?;

            // Convert metadata HashMap to a Python dict
            let metadata_dict = PyDict::new(py);
            for (key, value) in metadata {
                metadata_dict.set_item(key, value)?;
            }
            auth_dict.set_item("metadata", metadata_dict)?;
        }
        PorAuthEvent::InboundAuthSuccess {
            peer_id,
            connection_id,
            address,
        } => {
            auth_dict.set_item("type", "InboundAuthSuccess")?;
            auth_dict.set_item("peer_id", peer_id.to_string())?;
            // Convert connection_id to integer
            let conn_id_value = connection_id_to_u64(connection_id);
            auth_dict.set_item("connection_id", conn_id_value)?;
            auth_dict.set_item("address", address.to_string())?;
        }
        PorAuthEvent::OutboundAuthFailure {
            peer_id,
            connection_id,
            address,
            reason,
        } => {
            auth_dict.set_item("type", "OutboundAuthFailure")?;
            auth_dict.set_item("peer_id", peer_id.to_string())?;
            // Convert connection_id to integer
            let conn_id_value = connection_id_to_u64(connection_id);
            auth_dict.set_item("connection_id", conn_id_value)?;
            auth_dict.set_item("address", address.to_string())?;
            auth_dict.set_item("reason", reason)?;
        }
        PorAuthEvent::InboundAuthFailure {
            peer_id,
            connection_id,
            address,
            reason,
        } => {
            auth_dict.set_item("type", "InboundAuthFailure")?;
            auth_dict.set_item("peer_id", peer_id.to_string())?;
            // Convert connection_id to integer
            let conn_id_value = connection_id_to_u64(connection_id);
            auth_dict.set_item("connection_id", conn_id_value)?;
            auth_dict.set_item("address", address.to_string())?;
            auth_dict.set_item("reason", reason)?;
        }
        PorAuthEvent::AuthTimeout {
            peer_id,
            connection_id,
            address,
            direction,
        } => {
            auth_dict.set_item("type", "AuthTimeout")?;
            auth_dict.set_item("peer_id", peer_id.to_string())?;
            // Convert connection_id to integer
            let conn_id_value = connection_id_to_u64(connection_id);
            auth_dict.set_item("connection_id", conn_id_value)?;
            auth_dict.set_item("address", address.to_string())?;
            // Convert direction enum to a string representation
            let direction_str = match direction {
                // Replace with actual variants of AuthDirection
                _ => "Unknown", // Fallback value - replace with actual enum handling
            };
            auth_dict.set_item("direction", direction_str)?;
        }
    }

    Ok(auth_dict.to_object(py))
}
