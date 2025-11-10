#![allow(warnings)]
pub mod behaviour;
pub mod consts;
pub mod events;
pub mod handler;
pub mod handshake;
pub mod header;
pub mod pending_streams;
pub mod protocol;
pub mod types;
pub mod utils;
pub mod xstream_state;
pub mod xstream;
pub mod error_handling;
pub mod xstream_error;
// Добавьте следующее для подключения тестов:
#[cfg(test)]
mod tests;
