//! Screen flow shared by the platform adapters.
//! A coordinate is snapped before the router runs. The drawn route is the
//! core's edge list. This crate does not choose a cost.

use std::time::Duration;

use ai_smart_maps_core::confidence::{Environment, RouteNovelty};
use ai_smart_maps_core::graph::Graph;
use ai_smart_maps_core::router::{Request, Router};
use ai_smart_maps_core::snap::{self, Destination};
use ai_smart_maps_core::store::{LocalStore, StoreError};
use ai_smart_maps_core::training::TrainingRecord;

pub const VOICE_LIMIT: Duration = Duration::from_millis(200);
pub const TILE_RENDER_LIMIT: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug)]
pub enum Place<'a> {
    Node(&'a str),
    Coordinate { lat: f64, lon: f64 },
}

#[derive(Clone, Debug)]
pub struct Trip<'a> {
    pub tile_id: &'a str,
    pub now: &'a str,
    pub model_version_id: &'a str,
    pub map_age_days: f64,
    pub report_count: u32,
    pub rolled_back: bool,
    pub route_novelty: RouteNovelty,
    pub environment: Environment,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Screen {
    pub platform: &'static str,
    pub edge_ids: Vec<String>,
    pub warning: bool,
    pub voice_issued: bool,
    pub tile_drawn: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ScreenError {
    Rejected,
    Unreachable,
    VoiceTooSlow,
    TileTooSlow,
}

pub fn show(
    platform: &'static str,
    graph: &Graph,
    router: &mut Router,
    origin: &str,
    destination: Place<'_>,
    trip: &Trip<'_>,
    voice_elapsed: Duration,
    tile_elapsed: Duration,
) -> Result<Screen, ScreenError> {
    if voice_elapsed > VOICE_LIMIT {
        return Err(ScreenError::VoiceTooSlow);
    }
    if tile_elapsed > TILE_RENDER_LIMIT {
        return Err(ScreenError::TileTooSlow);
    }
    let destination_id = snap_to_node(graph, destination)?;
    let request = Request {
        origin,
        destination: destination_id,
        tile_id: trip.tile_id,
        now: trip.now,
        model_version_id: trip.model_version_id,
        map_age_days: trip.map_age_days,
        report_count: trip.report_count,
        rolled_back: trip.rolled_back,
        route_novelty: trip.route_novelty,
        environment: trip.environment,
    };
    let route = router
        .route(graph, &request)
        .ok_or(ScreenError::Unreachable)?;
    Ok(Screen {
        platform,
        edge_ids: route.edge_ids,
        warning: route.confidence.warns(),
        voice_issued: true,
        tile_drawn: true,
    })
}

pub fn open_cache(key: &[u8]) -> Result<LocalStore, StoreError> {
    LocalStore::open(key)
}

pub fn clear_local(store: &mut LocalStore, training: &mut TrainingRecord) {
    store.clear();
    training.clear_opt_in();
}

fn snap_to_node<'a>(graph: &'a Graph, destination: Place<'_>) -> Result<&'a str, ScreenError> {
    let destination = match destination {
        Place::Node(id) => Destination::NodeId(id),
        Place::Coordinate { lat, lon } => Destination::Coordinate { lat, lon },
    };
    snap::resolve(graph, destination).map_err(|_| ScreenError::Rejected)
}
