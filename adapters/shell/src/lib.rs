//! Screen flow shared by the platform adapters. Every screen comes from
//! `Device`, so tile admission, artifact rollback, snapping, and sealing
//! run before anything is drawn. This crate does not choose a cost.

use std::time::Duration;

pub use ai_smart_maps_core::device::{Device, DeviceError, Trip};
pub use ai_smart_maps_core::reports::Kind;
use ai_smart_maps_core::snap::Destination;
use ai_smart_maps_core::store::StoreError;

pub const VOICE_LIMIT: Duration = Duration::from_millis(200);
pub const TILE_RENDER_LIMIT: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug)]
pub enum Place<'a> {
    Node(&'a str),
    Coordinate { lat: f64, lon: f64 },
}

/// How long the platform took to speak and to draw, measured by the adapter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Timing {
    pub voice: Duration,
    pub tile: Duration,
}

/// The sealed trip a mid-trip screen continues: the original origin and
/// destination, the tile that holds them, and the new position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Continue<'a> {
    pub origin: &'a str,
    pub destination: &'a str,
    pub tile_id: &'a str,
    pub from: &'a str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Screen {
    pub platform: &'static str,
    pub tile_id: String,
    pub edge_ids: Vec<String>,
    pub warning: bool,
    pub voice_issued: bool,
    pub tile_drawn: bool,
}

/// What the platform can speak after a report is sealed: the edge and
/// the kind. No reporter key, no detail.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Filed {
    pub edge_id: String,
    pub kind: Kind,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ScreenError {
    Device(DeviceError),
    VoiceTooSlow,
    TileTooSlow,
}

/// Opens the device with the secure-store key and the map-source key.
pub fn open(cache_key: &[u8], map_source_key: &[u8]) -> Result<Device, StoreError> {
    Device::open(cache_key, map_source_key)
}

pub fn show(
    platform: &'static str,
    device: &mut Device,
    origin: &str,
    destination: Place<'_>,
    trip: &Trip<'_>,
    timing: Timing,
) -> Result<Screen, ScreenError> {
    within_limits(timing)?;
    let destination = match destination {
        Place::Node(id) => Destination::NodeId(id),
        Place::Coordinate { lat, lon } => Destination::Coordinate { lat, lon },
    };
    let outcome = device
        .route(origin, destination, trip)
        .map_err(ScreenError::Device)?;
    Ok(screen(platform, outcome))
}

/// Continues a sealed trip from a new position. Same voice and tile
/// limits as [`show`]. The destination is the node already on the sealed
/// route, not a fresh snap.
pub fn show_reroute(
    platform: &'static str,
    device: &mut Device,
    along: Continue<'_>,
    trip: &Trip<'_>,
    timing: Timing,
) -> Result<Screen, ScreenError> {
    within_limits(timing)?;
    let outcome = device
        .reroute(
            along.origin,
            along.destination,
            along.tile_id,
            along.from,
            trip,
        )
        .map_err(ScreenError::Device)?;
    Ok(screen(platform, outcome))
}

fn within_limits(timing: Timing) -> Result<(), ScreenError> {
    if timing.voice > VOICE_LIMIT {
        return Err(ScreenError::VoiceTooSlow);
    }
    if timing.tile > TILE_RENDER_LIMIT {
        return Err(ScreenError::TileTooSlow);
    }
    Ok(())
}

fn screen(platform: &'static str, outcome: ai_smart_maps_core::device::Outcome) -> Screen {
    Screen {
        platform,
        tile_id: outcome.tile_id,
        warning: outcome.route.confidence.warns(),
        edge_ids: outcome.route.edge_ids,
        voice_issued: true,
        tile_drawn: true,
    }
}

/// The sealed cache as one encrypted blob for the platform to write to
/// its app storage. Still encrypted under the secure-store key.
pub fn save(device: &mut Device) -> Result<Vec<u8>, StoreError> {
    device.export_all()
}

/// Reopens the device from a blob written by [`save`]. Tiles, scorers,
/// routes, reports, and the reporter key come back. A blob sealed under
/// another key is refused.
pub fn restore(cache_key: &[u8], map_source_key: &[u8], blob: &[u8]) -> Result<Device, StoreError> {
    let mut device = open(cache_key, map_source_key)?;
    device.import_all(blob)?;
    Ok(device)
}

/// A local report from this device. An unknown edge is `ScreenError`.
/// The next [`show`] of a closed remaining edge is not the previous list.
/// The returned [`Filed`] names the edge and the kind so the platform
/// can confirm the seal without reading the core observation.
pub fn report(
    device: &mut Device,
    edge_id: &str,
    kind: Kind,
    detail: Option<&str>,
    trip: &Trip<'_>,
) -> Result<Filed, ScreenError> {
    let observation = device
        .report(edge_id, kind, detail, trip)
        .map_err(ScreenError::Device)?;
    Ok(Filed {
        edge_id: observation.edge_id,
        kind: observation.kind,
    })
}

/// The clear-data action. Sealed routes and the training opt-in go.
pub fn clear_local(device: &mut Device) {
    device.clear_local_data();
}
