//! Routing core for one device.
//!
//! The router reads a graph and returns a route. It does not import a map
//! adapter, open a network connection, or store an account.

pub mod adapt;
pub mod confidence;
pub mod graph;
pub mod reports;
pub mod router;
