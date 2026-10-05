//! On-device training pairs. A pair is stored only after opt-in.
//! Clearing the opt-in deletes the pairs and the model version.
//! The record holds feature numbers and a model version id, capped at
//! [`TRAINING_PAIR_CAP`] pairs; the oldest pair leaves first.

use crate::scorer::Features;

/// The most pairs one device keeps. At 40 bytes of numbers each this is
/// under half a megabyte, well inside the maps-and-models budget.
pub const TRAINING_PAIR_CAP: usize = 10_000;

#[derive(Clone, Debug, PartialEq)]
pub struct TrainingPair {
    pub left: Features,
    pub right: Features,
    pub left_has_lower_cost: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrainingRecord {
    opted_in: bool,
    pairs: Vec<TrainingPair>,
    model_version_id: Option<String>,
    cap: usize,
}

impl Default for TrainingRecord {
    fn default() -> Self {
        Self::with_cap(TRAINING_PAIR_CAP)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct NotOptedIn;

impl TrainingRecord {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_cap(cap: usize) -> Self {
        Self {
            opted_in: false,
            pairs: Vec::new(),
            model_version_id: None,
            cap,
        }
    }

    pub fn cap(&self) -> usize {
        self.cap
    }

    pub fn opt_in(&mut self, model_version_id: &str) {
        self.opted_in = true;
        self.model_version_id = Some(model_version_id.to_string());
    }

    /// Keeps the pair when opted in. A pair equal to the last one is not
    /// kept twice. Past the cap the oldest pair leaves.
    pub fn record(&mut self, pair: TrainingPair) -> Result<(), NotOptedIn> {
        if !self.opted_in {
            return Err(NotOptedIn);
        }
        if self.pairs.last() == Some(&pair) {
            return Ok(());
        }
        self.pairs.push(pair);
        while self.pairs.len() > self.cap {
            self.pairs.remove(0);
        }
        Ok(())
    }

    /// Deletes the pairs and the model version, and leaves the record opted out.
    pub fn clear_opt_in(&mut self) {
        self.opted_in = false;
        self.pairs.clear();
        self.model_version_id = None;
    }

    pub fn opted_in(&self) -> bool {
        self.opted_in
    }

    pub fn pairs(&self) -> &[TrainingPair] {
        &self.pairs
    }

    pub fn model_version_id(&self) -> Option<&str> {
        self.model_version_id.as_deref()
    }
}
