//! Screen flow shared by the platform adapters. Every screen comes from
//! `Device`, so tile admission, artifact rollback, snapping, and sealing
//! run before anything is drawn. This crate does not choose a cost.

use std::time::Duration;

pub use ai_smart_maps_core::device::{Device, DeviceError, Trip};
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Screen {
    pub platform: &'static str,
    pub tile_id: String,
    pub edge_ids: Vec<String>,
    pub warning: bool,
    pub voice_issued: bool,
    pub tile_drawn: bool,
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
    if timing.voice > VOICE_LIMIT {
        return Err(ScreenError::VoiceTooSlow);
    }
    if timing.tile > TILE_RENDER_LIMIT {
        return Err(ScreenError::TileTooSlow);
    }
    let destination = match destination {
        Place::Node(id) => Destination::NodeId(id),
        Place::Coordinate { lat, lon } => Destination::Coordinate { lat, lon },
    };
    let outcome = device
        .route(origin, destination, trip)
        .map_err(ScreenError::Device)?;
    Ok(Screen {
        platform,
        tile_id: outcome.tile_id,
        warning: outcome.route.confidence.warns(),
        edge_ids: outcome.route.edge_ids,
        voice_issued: true,
        tile_drawn: true,
    })
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

/// The clear-data action. Sealed routes and the training opt-in go.
pub fn clear_local(device: &mut Device) {
    device.clear_local_data();
}
