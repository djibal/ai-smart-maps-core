//! Maps and models share one byte budget. The default is 2GB.
//! When an insert would pass that budget, the oldest tile leaves first.
//! The active scorer, and the artifact named by its `previous_id`, stay
//! while rollback is still required.

use std::cmp::Ordering;

pub const MAPS_AND_MODELS_BUDGET: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
struct TileSlot {
    id: String,
    observed_at: String,
    bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ModelSlot {
    id: String,
    created_at: String,
    bytes: u64,
    previous_id: Option<String>,
    active: bool,
}

#[derive(Clone, Debug)]
pub struct Budget {
    limit: u64,
    tiles: Vec<TileSlot>,
    models: Vec<ModelSlot>,
}

impl Default for Budget {
    fn default() -> Self {
        Self::with_limit(MAPS_AND_MODELS_BUDGET)
    }
}

impl Budget {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_limit(limit: u64) -> Self {
        Self {
            limit,
            tiles: Vec::new(),
            models: Vec::new(),
        }
    }

    pub fn limit(&self) -> u64 {
        self.limit
    }

    pub fn used(&self) -> u64 {
        self.tiles.iter().map(|tile| tile.bytes).sum::<u64>()
            + self.models.iter().map(|model| model.bytes).sum::<u64>()
    }

    pub fn tile_ids(&self) -> Vec<String> {
        let mut tiles = self.tiles.clone();
        tiles.sort_by(|left, right| {
            by_time_then_id(&left.observed_at, &left.id, &right.observed_at, &right.id)
        });
        tiles.into_iter().map(|tile| tile.id).collect()
    }

    pub fn model_ids(&self) -> Vec<String> {
        let mut models = self.models.clone();
        models.sort_by(|left, right| {
            by_time_then_id(&left.created_at, &left.id, &right.created_at, &right.id)
        });
        models.into_iter().map(|model| model.id).collect()
    }

    /// Keeps one record per tile id. A later `observed_at` replaces that
    /// record. An earlier one leaves it unchanged.
    pub fn insert_tile(&mut self, id: &str, observed_at: &str, bytes: u64) {
        if let Some(existing) = self.tiles.iter_mut().find(|tile| tile.id == id) {
            if observed_at < existing.observed_at.as_str() {
                return;
            }
            existing.observed_at = observed_at.to_string();
            existing.bytes = bytes;
        } else {
            self.tiles.push(TileSlot {
                id: id.to_string(),
                observed_at: observed_at.to_string(),
                bytes,
            });
        }
        self.evict();
    }

    /// Counts a scorer artifact toward the same budget. `active` with a
    /// `previous_id` keeps both artifacts while rollback is required.
    pub fn insert_model(
        &mut self,
        id: &str,
        created_at: &str,
        bytes: u64,
        previous_id: Option<&str>,
        active: bool,
    ) {
        let previous_id = previous_id.map(str::to_string);
        if let Some(existing) = self.models.iter_mut().find(|model| model.id == id) {
            existing.created_at = created_at.to_string();
            existing.bytes = bytes;
            existing.previous_id = previous_id;
            existing.active = active;
        } else {
            self.models.push(ModelSlot {
                id: id.to_string(),
                created_at: created_at.to_string(),
                bytes,
                previous_id,
                active,
            });
        }
        self.evict();
    }

    fn evict(&mut self) {
        while self.used() > self.limit {
            if let Some(index) = self.oldest_tile() {
                self.tiles.remove(index);
                continue;
            }
            let Some(index) = self.oldest_evictable_model() else {
                break;
            };
            self.models.remove(index);
        }
    }

    fn oldest_tile(&self) -> Option<usize> {
        self.tiles
            .iter()
            .enumerate()
            .min_by(|(_, left), (_, right)| {
                by_time_then_id(&left.observed_at, &left.id, &right.observed_at, &right.id)
            })
            .map(|(index, _)| index)
    }

    fn oldest_evictable_model(&self) -> Option<usize> {
        let protected = self.protected_model_ids();
        self.models
            .iter()
            .enumerate()
            .filter(|(_, model)| !protected.contains(&model.id))
            .min_by(|(_, left), (_, right)| {
                by_time_then_id(&left.created_at, &left.id, &right.created_at, &right.id)
            })
            .map(|(index, _)| index)
    }

    fn protected_model_ids(&self) -> Vec<String> {
        let mut ids = Vec::new();
        for model in &self.models {
            if model.active {
                if let Some(previous) = &model.previous_id {
                    ids.push(model.id.clone());
                    ids.push(previous.clone());
                }
            }
        }
        ids
    }
}

fn by_time_then_id(left_time: &str, left_id: &str, right_time: &str, right_id: &str) -> Ordering {
    left_time
        .cmp(right_time)
        .then_with(|| left_id.cmp(right_id))
}
