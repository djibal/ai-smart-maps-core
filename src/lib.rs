//! Routing core for one device.
//!
//! The router reads a graph and returns a route. It does not import a map
//! adapter, open a network connection, or store an account.

pub mod adapt;
pub mod budget;
pub mod confidence;
pub mod contracts;
pub mod graph;
pub mod log;
pub mod reports;
pub mod router;
pub mod scorer;
pub mod snap;
pub mod store;
pub mod training;
