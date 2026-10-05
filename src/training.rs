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

    /// Bytes for the sealed cache. Opt-in flag, model version, cap, then
    /// the pairs as eight numbers and a flag each. Not a contract.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = vec![u8::from(self.opted_in)];
        let version = self.model_version_id.as_deref().unwrap_or("");
        out.extend((version.len() as u32).to_be_bytes());
        out.extend(version.as_bytes());
        out.extend((self.cap as u64).to_be_bytes());
        out.extend((self.pairs.len() as u64).to_be_bytes());
        for pair in &self.pairs {
            for features in [&pair.left, &pair.right] {
                out.extend(features.edge_count.to_be_bytes());
                out.extend(features.weight_sum.to_be_bytes());
                out.extend(features.hazard_sum.to_be_bytes());
                out.extend(features.hazard_missing.to_be_bytes());
            }
            out.push(u8::from(pair.left_has_lower_cost));
        }
        out
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        let mut reader = Reader(bytes);
        let opted_in = flag(reader.byte()?)?;
        let version_len = reader.u32()? as usize;
        let version = String::from_utf8(reader.take(version_len)?.to_vec()).ok()?;
        let cap = usize::try_from(reader.u64()?).ok()?;
        let count = usize::try_from(reader.u64()?).ok()?;
        let mut pairs = Vec::with_capacity(count.min(TRAINING_PAIR_CAP));
        for _ in 0..count {
            let left = reader.features()?;
            let right = reader.features()?;
            let left_has_lower_cost = flag(reader.byte()?)?;
            pairs.push(TrainingPair {
                left,
                right,
                left_has_lower_cost,
            });
        }
        if !reader.0.is_empty() || pairs.len() > cap || (!opted_in && !pairs.is_empty()) {
            return None;
        }
        Some(Self {
            opted_in,
            pairs,
            model_version_id: (!version.is_empty()).then_some(version),
            cap,
        })
    }

    pub fn pairs(&self) -> &[TrainingPair] {
        &self.pairs
    }

    pub fn model_version_id(&self) -> Option<&str> {
        self.model_version_id.as_deref()
    }
}

fn flag(byte: u8) -> Option<bool> {
    match byte {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn take(&mut self, len: usize) -> Option<&[u8]> {
        let (head, rest) = self.0.split_at_checked(len)?;
        self.0 = rest;
        Some(head)
    }

    fn byte(&mut self) -> Option<u8> {
        self.take(1).map(|b| b[0])
    }

    fn u32(&mut self) -> Option<u32> {
        self.take(4)?.try_into().ok().map(u32::from_be_bytes)
    }

    fn u64(&mut self) -> Option<u64> {
        self.take(8)?.try_into().ok().map(u64::from_be_bytes)
    }

    fn f64(&mut self) -> Option<f64> {
        self.take(8)?.try_into().ok().map(f64::from_be_bytes)
    }

    fn features(&mut self) -> Option<Features> {
        Some(Features {
            edge_count: self.u32()?,
            weight_sum: self.f64()?,
            hazard_sum: self.f64()?,
            hazard_missing: self.u32()?,
        })
    }
}
