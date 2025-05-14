#![allow(warnings)]
// src/lib.rs
// src/lib.rs
use pyo3::prelude::*;

mod network;
mod py;

#[pymodule]
fn p2p_network_py(py: Python, m: &PyModule) -> PyResult<()> {
    // Current best way to initialize the tokio runtime for PyO3
    let _ = pyo3_asyncio::tokio::future_into_py(py, async {
        // Just a dummy future to trigger initialization
        Ok(())
    });
    
    // Register the Node class
    m.add_class::<py::node::Node>()?;
    
    // Register the XStream class
    m.add_class::<py::xstream::XStream>()?;
    
    // Register utility types
    m.add_class::<py::types::PeerId>()?;
    m.add_class::<py::types::KeyPair>()?;
    m.add_class::<py::types::ProofOfRepresentation>()?;
    m.add_class::<py::types::PorUtils>()?;
    
    
    // Add utility functions
    m.add_function(wrap_pyfunction!(py::types::generate_keypair, m)?)?;
    m.add_function(wrap_pyfunction!(py::types::peer_id_from_keypair, m)?)?;
    m.add_function(wrap_pyfunction!(py::types::generate_por, m)?)?;
    
    
    Ok(())
}