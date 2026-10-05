//! Tile admission and selection.
//!
//! A local tile may omit a signature. A signature, when present, is Ed25519
//! over the protobuf encoding of the tile with the signature omitted, checked
//! with the 32-byte map-source public key. A fetched tile must carry a
//! signature that passes that check. A failure leaves the cache unchanged.

use base64::{engine::general_purpose::STANDARD, Engine};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};

use crate::contracts::{encode_tile, TileRecord};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    Local,
    Fetched,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Rejected;

/// Protobuf bytes of the tile with `signature` omitted. That is the signed message.
pub fn signed_bytes(tile: &TileRecord) -> Result<Vec<u8>, crate::contracts::ContractError> {
    let mut unsigned = tile.clone();
    unsigned.signature = None;
    encode_tile(&unsigned)
}

/// True when `signature` is standard base64 of a 64-byte Ed25519 signature
/// and `map_source_key` is the matching 32-byte public key.
pub fn verify(tile: &TileRecord, map_source_key: &[u8]) -> bool {
    let Some(encoded) = tile.signature.as_deref() else {
        return false;
    };
    let Ok(key_bytes) = <[u8; 32]>::try_from(map_source_key) else {
        return false;
    };
    let Ok(key) = VerifyingKey::from_bytes(&key_bytes) else {
        return false;
    };
    let Ok(decoded) = STANDARD.decode(encoded) else {
        return false;
    };
    let Ok(signature) = Signature::from_slice(&decoded) else {
        return false;
    };
    let Ok(message) = signed_bytes(tile) else {
        return false;
    };
    key.verify(&message, &signature).is_ok()
}

pub fn admit(tile: &TileRecord, origin: Origin, map_source_key: &[u8]) -> Result<(), Rejected> {
    match tile.signature {
        None => match origin {
            Origin::Local => Ok(()),
            Origin::Fetched => Err(Rejected),
        },
        Some(_) => {
            if verify(tile, map_source_key) {
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
        map_source_key: &[u8],
    ) -> Result<(), Rejected> {
        admit(&tile, origin, map_source_key)?;
        if let Some(existing) = self.tiles.iter_mut().find(|stored| stored.id == tile.id) {
            if tile.observed_at.as_str() >= existing.observed_at.as_str() {
                *existing = tile;
            }
            return Ok(());
        }
        self.tiles.push(tile);
        Ok(())
    }

    /// Drops every tile whose id is not in `ids`. The budget decides which
    /// ids stay; this cache follows it.
    pub fn retain_ids(&mut self, ids: &[String]) {
        self.tiles.retain(|tile| ids.contains(&tile.id));
    }

    pub fn get(&self, id: &str) -> Option<&TileRecord> {
        self.tiles.iter().find(|tile| tile.id == id)
    }

    /// Every cached tile whose graph has the edge, for a report to change.
    pub fn with_edge_mut(&mut self, edge_id: &str) -> Vec<&mut TileRecord> {
        self.tiles
            .iter_mut()
            .filter(|tile| tile.graph.edges.iter().any(|edge| edge.id == edge_id))
            .collect()
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
