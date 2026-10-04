//! On-device training pairs. A pair is stored only after opt-in.
//! Clearing the opt-in deletes the pairs and the model version.
//! The record holds feature numbers and a model version id.

use crate::scorer::Features;

#[derive(Clone, Debug, PartialEq)]
pub struct TrainingPair {
    pub left: Features,
    pub right: Features,
    pub left_has_lower_cost: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TrainingRecord {
    opted_in: bool,
    pairs: Vec<TrainingPair>,
    model_version_id: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct NotOptedIn;

impl TrainingRecord {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn opt_in(&mut self, model_version_id: &str) {
        self.opted_in = true;
        self.model_version_id = Some(model_version_id.to_string());
    }

    pub fn record(&mut self, pair: TrainingPair) -> Result<(), NotOptedIn> {
        if !self.opted_in {
            return Err(NotOptedIn);
        }
        self.pairs.push(pair);
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
