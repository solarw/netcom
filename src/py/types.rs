// src/py/types.rs
use pyo3::prelude::*;
use libp2p::{identity, PeerId as LibP2PPeerId};
use std::str::FromStr;
use std::fmt;

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
    
    fn to_peer_id(&self) -> PeerId {
        PeerId {
            inner: self.inner.public().to_peer_id(),
        }
    }
    
    fn __str__(&self) -> PyResult<String> {
        Ok(format!("KeyPair with public key: {:?}", self.inner.public()))
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
            Err(e) => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                format!("Invalid PeerId string: {}", e),
            )),
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