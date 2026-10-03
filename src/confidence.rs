//! Interim confidence score. Calibration is still unmeasured.
//! A stale map, a rolled-back model, an untraveled route, or a complex
//! environment sets the score to 0.5.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouteNovelty {
    Known,
    Untraveled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Environment {
    Simple,
    Complex,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Confidence {
    pub score: f64,
    pub map_age_days: f64,
    pub report_count: u32,
    pub model_version_id: String,
    pub route_novelty: RouteNovelty,
    pub environment: Environment,
}

impl Confidence {
    pub fn interim(
        map_age_days: f64,
        report_count: u32,
        model_version_id: &str,
        rolled_back: bool,
        route_novelty: RouteNovelty,
        environment: Environment,
    ) -> Self {
        let age: f64 = if map_age_days <= 90.0 { 1.0 } else { 0.5 };
        let model: f64 = if rolled_back { 0.5 } else { 1.0 };
        let novelty: f64 = if route_novelty == RouteNovelty::Known {
            1.0
        } else {
            0.5
        };
        let setting: f64 = if environment == Environment::Simple {
            1.0
        } else {
            0.5
        };
        let score = age.min(1.0).min(model).min(novelty).min(setting);
        Self {
            score,
            map_age_days,
            report_count,
            model_version_id: model_version_id.to_string(),
            route_novelty,
            environment,
        }
    }

    pub fn warns(&self) -> bool {
        self.score < 0.6
    }
}
