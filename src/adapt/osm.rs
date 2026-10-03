use super::{emit, RoadRecord};
use crate::graph::{Graph, Source};

pub fn graph(records: &[RoadRecord]) -> Graph {
    emit(records, Source::Osm)
}
