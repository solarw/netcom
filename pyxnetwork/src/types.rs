// src/py/types.rs
use libp2p::{identity, PeerId as LibP2PPeerId};
use pyo3::prelude::*;
use std::fmt;
use std::str::FromStr;
use std::time::Duration;
use xauth::por::por;

#[pyclass]
#[derive(Clone, Debug)]
pub struct KeyPair {
    pub inner: identity::Keypair,
}

#[pymethods]
impl KeyPair {
    #[new]
    fn new() -> Self {
        Self {
            inner: identity::Keypair::generate_ed25519(),
        }
    }

    pub fn to_peer_id(&self) -> PeerId {
        PeerId {
            inner: self.inner.public().to_peer_id(),
        }
    }

    fn __str__(&self) -> PyResult<String> {
        Ok(format!(
            "KeyPair with public key: {:?}",
            self.inner.public()
        ))
    }

    fn __repr__(&self) -> PyResult<String> {
        self.__str__()
    }
}

#[pyclass]
#[derive(Clone, Debug)]
pub struct PeerId {
    pub inner: LibP2PPeerId,
}

#[pymethods]
impl PeerId {
    #[new]
    fn new(peer_id_str: &str) -> PyResult<Self> {
        match LibP2PPeerId::from_str(peer_id_str) {
            Ok(peer_id) => Ok(Self { inner: peer_id }),
            Err(e) => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                "Invalid PeerId string: {}",
                e
            ))),
        }
    }

    fn __str__(&self) -> PyResult<String> {
        Ok(self.inner.to_string())
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("PeerId(\"{}\")", self.inner.to_string()))
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.inner == other.inner
    }

    fn __hash__(&self) -> isize {
        // Simple hash implementation based on the string representation
        let s = self.inner.to_string();
        let mut hash: isize = 0;
        for c in s.bytes() {
            hash = (hash * 31 + c as isize) % (isize::MAX / 2);
        }
        hash
    }
}

// Implement Display for PeerId to make it easier to use with format!
impl fmt::Display for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[pyfunction]
pub fn generate_keypair() -> KeyPair {
    KeyPair::new()
}

#[pyfunction]
pub fn peer_id_from_keypair(keypair: &KeyPair) -> PeerId {
    keypair.to_peer_id()
}

#[pyclass]
#[derive(Clone, Debug)]
pub struct ProofOfRepresentation {
    pub inner: por::ProofOfRepresentation,
}

#[pymethods]
impl ProofOfRepresentation {
    #[new]
    fn new(
        owner_keypair: &KeyPair,
        peer_id: &PeerId,
        validity_duration_secs: u64,
    ) -> PyResult<Self> {
        let duration = Duration::from_secs(validity_duration_secs);
        match por::ProofOfRepresentation::create(
            &owner_keypair.inner,
            peer_id.inner.clone(),
            duration,
        ) {
            Ok(por) => Ok(Self { inner: por }),
            Err(e) => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                "Failed to create ProofOfRepresentation: {}",
                e
            ))),
        }
    }

    /// Create with specific timestamps (for testing)
    #[staticmethod]
    fn create_with_times(
        owner_keypair: &KeyPair,
        peer_id: &PeerId,
        issued_at: u64,
        expires_at: u64,
    ) -> PyResult<Self> {
        match por::ProofOfRepresentation::create_with_times(
            &owner_keypair.inner,
            peer_id.inner.clone(),
            issued_at,
            expires_at,
        ) {
            Ok(por) => Ok(Self { inner: por }),
            Err(e) => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                "Failed to create ProofOfRepresentation: {}",
                e
            ))),
        }
    }

    /// Validate the POR
    pub fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                "Invalid ProofOfRepresentation: {}",
                e
            ))
        })
    }

    /// Check if POR has expired
    pub fn is_expired(&self) -> PyResult<bool> {
        self.inner.is_expired().map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                "Failed to check expiration: {}",
                e
            ))
        })
    }

    /// Get remaining validity time in seconds
    pub fn remaining_time(&self) -> PyResult<Option<u64>> {
        self.inner.remaining_time().map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                "Failed to get remaining time: {}",
                e
            ))
        })
    }

    /// Get owner's public key
    pub fn owner_public_key(&self) -> Vec<u8> {
        self.inner.owner_public_key.encode_protobuf()
    }

    /// Get peer ID
    pub fn peer_id(&self) -> PeerId {
        PeerId {
            inner: self.inner.peer_id.clone(),
        }
    }

    /// Get issued at timestamp
    pub fn issued_at(&self) -> u64 {
        self.inner.issued_at
    }

    /// Get expires at timestamp
    pub fn expires_at(&self) -> u64 {
        self.inner.expires_at
    }

    fn __str__(&self) -> PyResult<String> {
        Ok(format!(
            "ProofOfRepresentation(owner: {}, peer: {}, issued: {}, expires: {})",
            self.inner.owner_public_key.to_peer_id(),
            self.inner.peer_id,
            self.inner.issued_at,
            self.inner.expires_at
        ))
    }

    fn __repr__(&self) -> PyResult<String> {
        self.__str__()
    }
}

#[pyclass]
#[derive(Clone, Debug)]
pub struct PorUtils;

#[pymethods]
impl PorUtils {
    #[staticmethod]
    pub fn generate_owner_keypair() -> KeyPair {
        KeyPair {
            inner: por::PorUtils::generate_owner_keypair(),
        }
    }

    #[staticmethod]
    pub fn keypair_from_bytes(secret_key_bytes: Vec<u8>) -> PyResult<KeyPair> {
        match por::PorUtils::keypair_from_bytes(&secret_key_bytes) {
            Ok(kp) => Ok(KeyPair { inner: kp }),
            Err(e) => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(e)),
        }
    }

    #[staticmethod]
    pub fn peer_id_from_keypair(keypair: &KeyPair) -> PeerId {
        PeerId {
            inner: por::PorUtils::peer_id_from_keypair(&keypair.inner),
        }
    }
}

#[pyfunction]
pub fn generate_por(
    owner_keypair: &KeyPair,
    peer_id: &PeerId,
    validity_duration_secs: u64,
) -> PyResult<ProofOfRepresentation> {
    ProofOfRepresentation::new(owner_keypair, peer_id, validity_duration_secs)
}

pub fn rust_por_to_py_por(py: Python, por_obj: por::ProofOfRepresentation) -> PyResult<PyObject> {
    // Create the Python ProofOfRepresentation instance
    let py_por = ProofOfRepresentation { inner: por_obj };

    // Convert to PyObject
    Ok(py_por.into_py(py))
}
