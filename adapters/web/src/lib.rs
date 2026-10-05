//! web adapter. It names the platform and calls the shared screen flow.

pub const PLATFORM: &str = "web";

pub use ai_smart_maps_shell::{
    clear_local, open, report, show, show_reroute, Continue, Device, DeviceError, Filed, Kind,
    Place, Screen, ScreenError, Timing, Trip, TILE_RENDER_LIMIT, VOICE_LIMIT,
};
