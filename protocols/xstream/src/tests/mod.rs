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

#[cfg(test)]
pub mod utils_tests;

#[cfg(test)]
pub mod xstream_error_comprehensive_tests;

#[cfg(test)]
pub mod xstream_error_coverage_tests;

#[cfg(test)]
pub mod pending_streams_coverage_tests;


#[cfg(test)]
pub mod read_to_end_close_quic_test;

#[cfg(test)]
pub mod inbound_upgrade_test;

#[cfg(test)]
pub mod inbound_upgrade_simple_test;

#[cfg(test)]
pub mod inbound_upgrade_comprehensive_tests;

#[cfg(test)]
pub mod xstream_simple_exchange_tests;

#[cfg(test)]
pub mod strict_test_utils;

#[cfg(test)]
pub mod strict_echo_test;

#[cfg(test)]
pub mod strict_boundary_tests;

#[cfg(test)]
pub mod strict_data_integrity_tests;

#[cfg(test)]
pub mod strict_isolation_tests;

#[cfg(test)]
pub mod close_read_test;

#[cfg(test)]
pub mod close_write_test;

#[cfg(test)]
pub mod close_read_quic_test;

#[cfg(test)]
pub mod decision_flow_test;

#[cfg(test)]
pub mod connection_reject_test;
