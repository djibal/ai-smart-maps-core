//! Tile admission and selection.
//!
//! A local tile may omit a signature. A signature, when present, must pass
//! the caller-supplied map-source check. A fetched tile must carry a
//! signature that passes that check. A failure leaves the cache unchanged.
//! This module does not name a signature scheme.

use crate::contracts::TileRecord;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    Local,
    Fetched,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Rejected;

pub fn admit(
    tile: &TileRecord,
    origin: Origin,
    verify: impl FnOnce(&TileRecord) -> bool,
) -> Result<(), Rejected> {
    match tile.signature {
        None => match origin {
            Origin::Local => Ok(()),
            Origin::Fetched => Err(Rejected),
        },
        Some(_) => {
            if verify(tile) {
                Ok(())
            } else {
                Err(Rejected)
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct TileCache {
    tiles: Vec<TileRecord>,
}

impl TileCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    /// Inserts only after [`admit`] succeeds. A newer `observed_at` replaces
    /// the same tile id. An older one leaves the stored tile in place.
    pub fn insert(
        &mut self,
        tile: TileRecord,
        origin: Origin,
        verify: impl FnOnce(&TileRecord) -> bool,
    ) -> Result<(), Rejected> {
        admit(&tile, origin, verify)?;
        if let Some(existing) = self.tiles.iter_mut().find(|stored| stored.id == tile.id) {
            if tile.observed_at.as_str() >= existing.observed_at.as_str() {
                *existing = tile;
            }
            return Ok(());
        }
        self.tiles.push(tile);
        Ok(())
    }

    /// The newest tile whose graph contains both node ids.
    /// Equal `observed_at` keeps the lexicographically smaller tile id.
    pub fn newest_covering(&self, origin: &str, destination: &str) -> Option<&TileRecord> {
        self.tiles
            .iter()
            .filter(|tile| tile.graph.has_node(origin) && tile.graph.has_node(destination))
            .max_by(|left, right| {
                left.observed_at
                    .cmp(&right.observed_at)
                    .then_with(|| right.id.cmp(&left.id))
            })
    }
}
