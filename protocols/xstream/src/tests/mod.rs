// tests/mod.rs
// Test module organization for XStream library

#[cfg(test)]
pub mod xstream_state_tests;

#[cfg(test)]
pub mod xstream_tests;

#[cfg(test)]
pub mod xstream_coverage_tests;

#[cfg(test)]
pub mod xstream_edge_tests;

#[cfg(test)]
pub mod error_handling_tests;

#[cfg(test)]
pub mod xstream_error_handling_tests;

#[cfg(test)]
pub mod xstream_diagnostics_tests;

#[cfg(test)]
pub mod pending_streams_unit_tests;

#[cfg(test)]
pub mod real_network_swarm_tests;

#[cfg(test)]
pub mod xstream_data_exchange_tests;

#[cfg(test)]
pub mod real_xstream_exchange_tests;

#[cfg(test)]
pub mod connection_handler_comprehensive_tests;

#[cfg(test)]
pub mod pending_streams_edge_cases_tests;

#[cfg(test)]
pub mod connection_handler_init_tests;

#[cfg(test)]
pub mod connection_handler_events_tests;

#[cfg(test)]
pub mod connection_handler_substream_tests;

#[cfg(test)]
pub mod connection_handler_outbound_tests;
