//! Destination resolution before the router runs.
//! A node id that exists in the graph is kept. A coordinate in range
//! becomes the nearest node id. Anything else is rejected.

use crate::graph::Graph;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Destination<'a> {
    NodeId(&'a str),
    Coordinate { lat: f64, lon: f64 },
}

#[derive(Debug, PartialEq, Eq)]
pub struct Rejected;

/// Returns the node id the router should use. This function is not the router.
pub fn resolve<'a>(graph: &'a Graph, destination: Destination<'_>) -> Result<&'a str, Rejected> {
    match destination {
        Destination::NodeId(id) => {
            if id.is_empty() {
                return Err(Rejected);
            }
            graph
                .nodes
                .iter()
                .find(|node| node.id == id)
                .map(|node| node.id.as_str())
                .ok_or(Rejected)
        }
        Destination::Coordinate { lat, lon } => {
            if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
                return Err(Rejected);
            }
            graph
                .nodes
                .iter()
                .min_by(|left, right| {
                    let left_distance = distance_m(lat, lon, left.lat, left.lon);
                    let right_distance = distance_m(lat, lon, right.lat, right.lon);
                    left_distance
                        .total_cmp(&right_distance)
                        .then_with(|| left.id.cmp(&right.id))
                })
                .map(|node| node.id.as_str())
                .ok_or(Rejected)
        }
    }
}

fn distance_m(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const EARTH_RADIUS_M: f64 = 6_371_000.0;
    let phi1 = lat1.to_radians();
    let phi2 = lat2.to_radians();
    let dphi = (lat2 - lat1).to_radians();
    let dlambda = (lon2 - lon1).to_radians();
    let a = (dphi / 2.0).sin().powi(2) + phi1.cos() * phi2.cos() * (dlambda / 2.0).sin().powi(2);
    EARTH_RADIUS_M * 2.0 * a.sqrt().asin()
}
