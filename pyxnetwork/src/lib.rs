// src/lib.rs
#![allow(warnings)]
pub mod events;
pub mod node;
pub mod types;
pub mod xstream_types;
pub mod xstream;

use pyo3::prelude::*;

#[pymodule]
fn pyxnetwork(py: Python, m: &PyModule) -> PyResult<()> {
    // Current best way to initialize the tokio runtime for PyO3
    let _ = pyo3_asyncio::tokio::future_into_py(py, async {
        // Just a dummy future to trigger initialization
        Ok(())
    });

    // Register the Node class
    m.add_class::<node::Node>()?;

    // Register the XStream class and related error types
    m.add_class::<xstream::XStream>()?;
    
    // Register utility types
    m.add_class::<types::PeerId>()?;
    m.add_class::<types::KeyPair>()?;
    m.add_class::<types::ProofOfRepresentation>()?;
    m.add_class::<types::PorUtils>()?;

    // Add utility functions
    m.add_function(wrap_pyfunction!(types::generate_keypair, m)?)?;
    m.add_function(wrap_pyfunction!(types::peer_id_from_keypair, m)?)?;
    m.add_function(wrap_pyfunction!(types::generate_por, m)?)?;

    // Register XStream types with correct names
    m.add_class::<xstream_types::PyXStreamError>()?;
    m.add_class::<xstream_types::PyErrorOnRead>()?;
    m.add_class::<xstream_types::StreamDirection>()?;
    m.add_class::<xstream_types::StreamState>()?;
    m.add_class::<xstream_types::XStreamErrorUtils>()?;

    Ok(())
}