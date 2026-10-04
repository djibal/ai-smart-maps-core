//! Web adapter. It names the platform and calls the shared screen flow.

pub const PLATFORM: &str = "web";

pub use ai_smart_maps_shell::{
    clear_local, open_cache, show, Place, Screen, ScreenError, Trip, TILE_RENDER_LIMIT, VOICE_LIMIT,
};
