//! Format-agnostic graph. Field names match the published graph schema.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Constraint {
    Open,
    Closed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Osm,
    Commercial,
    Custom,
    Realtime,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub id: String,
    pub lat: f64,
    pub lon: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Edge {
    pub id: String,
    pub src: String,
    pub dst: String,
    pub weight: f64,
    pub constraint: Constraint,
    pub source: Source,
    pub hazard: Option<f64>,
    pub valid_from: Option<String>,
    pub valid_to: Option<String>,
}

impl Edge {
    pub fn hazard_or_zero(&self) -> f64 {
        self.hazard.unwrap_or(0.0)
    }

    pub fn cost(&self) -> f64 {
        self.weight + (3.0 * self.hazard_or_zero())
    }

    /// An edge is excluded when it is closed, or when `now` falls outside a
    /// bound that is set. `now`, `valid_from`, and `valid_to` are comparable
    /// timestamps, such as UTC RFC3339.
    pub fn traversable_at(&self, now: &str) -> bool {
        if self.constraint == Constraint::Closed {
            return false;
        }
        if let Some(from) = &self.valid_from {
            if now < from.as_str() {
                return false;
            }
        }
        if let Some(to) = &self.valid_to {
            if now > to.as_str() {
                return false;
            }
        }
        true
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

impl Graph {
    pub fn has_node(&self, id: &str) -> bool {
        self.nodes.iter().any(|node| node.id == id)
    }
}
