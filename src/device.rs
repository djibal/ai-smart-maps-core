//! One device, end to end. A tile is admitted, the newest covering tile
//! is chosen, the destination is snapped, the router runs, the scorer
//! ranks, confidence is attached, the route is sealed, and the log is
//! written. The deterministic cost decides the route. The scorer does not.
//! Nothing here opens a network or stores a person.

use crate::budget::Budget;
use crate::confidence::{Environment, RouteNovelty};
use crate::contracts::{encode_route, encode_tile, ModelVersionRecord, TileRecord};
use crate::graph::{Constraint, Edge, Graph};
use crate::log::{DiagnosticLog, ErrorCode};
use crate::reporter::{self, ReporterKey};
use crate::reports::{self, AgedReport, Kind, Observation};
use crate::router::{Request, Route, Router};
use crate::scorer::{features, Features, Host, Prediction};
use crate::snap::{self, Destination};
use crate::store::{LocalStore, StoreError};
use crate::tiles::{Origin, TileCache};
use crate::training::{TrainingPair, TrainingRecord};

#[derive(Clone, Debug)]
pub struct Trip<'a> {
    pub now: &'a str,
    pub map_age_days: f64,
    pub report_count: u32,
    pub route_novelty: RouteNovelty,
    pub environment: Environment,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    pub tile_id: String,
    pub route: Route,
    pub prediction: Option<Prediction>,
    pub rolled_back: bool,
    pub sealed: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum DeviceError {
    TileRejected,
    NoScorer,
    NoTile,
    Rejected,
    Unreachable,
}

#[derive(Clone, Debug)]
struct ActiveScorer {
    id: String,
    previous_id: Option<String>,
    presented: Vec<u8>,
}

pub struct Device {
    tiles: TileCache,
    budget: Budget,
    router: Router,
    scorer: Host,
    store: LocalStore,
    training: TrainingRecord,
    map_source_key: Vec<u8>,
    active: Option<ActiveScorer>,
    reports: DiagnosticLog,
}

impl Device {
    /// `cache_key` is the 32-byte key from the platform secure store.
    /// `map_source_key` is the 32-byte Ed25519 public key of the map source.
    pub fn open(cache_key: &[u8], map_source_key: &[u8]) -> Result<Self, StoreError> {
        Ok(Self {
            tiles: TileCache::new(),
            budget: Budget::new(),
            router: Router::new(),
            scorer: Host::new(),
            store: LocalStore::open(cache_key)?,
            training: TrainingRecord::new(),
            map_source_key: map_source_key.to_vec(),
            active: None,
            reports: DiagnosticLog::new(),
        })
    }

    pub fn with_budget(mut self, budget: Budget) -> Self {
        self.budget = budget;
        self
    }

    /// Admits the tile, counts it toward the budget, and drops whatever the
    /// budget evicted. A rejected tile changes nothing.
    pub fn load_tile(&mut self, tile: TileRecord, origin: Origin) -> Result<(), DeviceError> {
        let bytes = encode_tile(&tile).map_err(|_| DeviceError::TileRejected)?;
        let id = tile.id.clone();
        let observed_at = tile.observed_at.clone();
        self.tiles
            .insert(tile, origin, &self.map_source_key)
            .map_err(|_| DeviceError::TileRejected)?;
        self.budget
            .insert_tile(&id, &observed_at, bytes.len() as u64);
        let kept = self.budget.tile_ids();
        self.tiles.retain_ids(&kept);
        let _ = self.store.put(&tile_key(&id), &bytes);
        for name in self.store.names_with_prefix("tile:") {
            if !kept.iter().any(|id| tile_key(id) == name) {
                self.store.remove(&name);
            }
        }
        Ok(())
    }

    /// Installs the artifact named by the model version and makes it active.
    /// The artifact and its record are sealed so a reopened device has them.
    pub fn install_scorer(&mut self, record: &ModelVersionRecord, bytes: &[u8]) {
        self.scorer.install(&record.id, bytes);
        self.budget.insert_model(
            &record.id,
            &record.created_at,
            bytes.len() as u64,
            record.previous_id.as_deref(),
            true,
        );
        self.active = Some(ActiveScorer {
            id: record.id.clone(),
            previous_id: record.previous_id.clone(),
            presented: bytes.to_vec(),
        });
        let _ = self.store.put(&model_key(&record.id), bytes);
        let _ = self
            .store
            .put(&model_record_key(&record.id), &encode_model_record(record));
        let _ = self.store.put(ACTIVE_RECORD, record.id.as_bytes());
    }

    /// The whole sealed cache as one encrypted blob for the platform to
    /// persist: tiles, scorer artifacts, routes, reports, and the reporter
    /// key. Still encrypted under the cache key.
    pub fn export_all(&self) -> Result<Vec<u8>, StoreError> {
        self.store.seal_all()
    }

    /// Rebuilds the device from a blob made by [`Self::export_all`] under
    /// the same cache key. Tiles and scorers come back in memory and in
    /// the budget. A blob from another key is refused and nothing changes.
    pub fn import_all(&mut self, blob: &[u8]) -> Result<(), StoreError> {
        self.store.open_all(blob)?;
        self.tiles = TileCache::new();
        self.budget = Budget::with_limit(self.budget.limit());
        self.scorer = Host::new();
        self.active = None;
        self.router = Router::new();

        for name in self.store.names_with_prefix("tile:") {
            let bytes = self.store.get(&name)?;
            let Ok(tile) = crate::contracts::decode_tile(&bytes) else {
                continue;
            };
            let id = tile.id.clone();
            let observed_at = tile.observed_at.clone();
            if self
                .tiles
                .insert(tile, Origin::Local, &self.map_source_key)
                .is_ok()
            {
                self.budget
                    .insert_tile(&id, &observed_at, bytes.len() as u64);
            }
        }
        self.tiles.retain_ids(&self.budget.tile_ids());

        let active_id = self
            .store
            .get(ACTIVE_RECORD)
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok());
        for name in self.store.names_with_prefix("model-record:") {
            let Some(record) = decode_model_record(&self.store.get(&name)?) else {
                continue;
            };
            let bytes = self.store.get(&model_key(&record.id))?;
            let active = active_id.as_deref() == Some(record.id.as_str());
            self.scorer.install(&record.id, &bytes);
            self.budget.insert_model(
                &record.id,
                &record.created_at,
                bytes.len() as u64,
                record.previous_id.as_deref(),
                active,
            );
            if active {
                self.active = Some(ActiveScorer {
                    id: record.id.clone(),
                    previous_id: record.previous_id.clone(),
                    presented: bytes,
                });
            }
        }
        Ok(())
    }

    /// The bytes the platform presents at startup. A later mismatch with
    /// the installed artifact rolls back to `previous_id`.
    pub fn present_artifact(&mut self, bytes: &[u8]) -> Result<(), DeviceError> {
        let active = self.active.as_mut().ok_or(DeviceError::NoScorer)?;
        active.presented = bytes.to_vec();
        Ok(())
    }

    pub fn route(
        &mut self,
        origin: &str,
        destination: Destination<'_>,
        trip: &Trip<'_>,
    ) -> Result<Outcome, DeviceError> {
        let active = self.active.clone().ok_or(DeviceError::NoScorer)?;
        let accepted = self.scorer.accepted_version(
            &active.id,
            &active.presented,
            active.previous_id.as_deref(),
        );
        let (model_version_id, rolled_back) = match accepted {
            Some(id) => {
                let rolled = id != active.id;
                (id, rolled)
            }
            None => (active.id.clone(), true),
        };

        let (tile_id, graph, destination_id) = {
            let (tile, destination_id) = self.resolve(origin, destination)?;
            (tile.id.clone(), tile.graph.clone(), destination_id)
        };

        let request = Request {
            origin,
            destination: &destination_id,
            tile_id: &tile_id,
            now: trip.now,
            model_version_id: &model_version_id,
            map_age_days: trip.map_age_days,
            report_count: trip.report_count,
            rolled_back,
            route_novelty: trip.route_novelty,
            environment: trip.environment,
        };
        let route = self
            .router
            .route(&graph, &request)
            .ok_or(DeviceError::Unreachable)?;

        let chosen = features(&graph, &route.edge_ids).ok();
        let prediction = chosen.as_ref().and_then(|found| {
            self.scorer
                .score(
                    &active.id,
                    &active.presented,
                    active.previous_id.as_deref(),
                    found,
                )
                .ok()
        });

        if self.training.opted_in() {
            if let Some(chosen) = chosen {
                self.collect_pair(&graph, &request, &route, chosen);
            }
        }

        let sealed = match encode_route(&route) {
            Ok(bytes) => self
                .store
                .put(&route_key(origin, &destination_id, &tile_id), &bytes)
                .is_ok(),
            Err(_) => false,
        };

        Ok(Outcome {
            tile_id,
            route,
            prediction,
            rolled_back,
            sealed,
        })
    }

    pub fn sealed_route(&self, origin: &str, destination: &str, tile_id: &str) -> Option<Route> {
        let bytes = self
            .store
            .get(&route_key(origin, destination, tile_id))
            .ok()?;
        crate::contracts::decode_route(&bytes).ok()
    }

    /// A local report from this device. The observation carries the
    /// reporter key, is applied to every cached tile that has the edge,
    /// drops the cached routes on those tiles, and is sealed under
    /// `report:{edge_id}:{now}`. An edge no cached tile
    /// knows is logged as `unknown_edge` and refused before sealing. The
    /// log gets the edge id and the duration, never the key or the detail.
    pub fn report(
        &mut self,
        edge_id: &str,
        kind: Kind,
        detail: Option<&str>,
        trip: &Trip<'_>,
    ) -> Result<Observation, DeviceError> {
        if edge_id.is_empty() {
            return Err(DeviceError::Rejected);
        }
        let key = self.reporter_key().map_err(|_| DeviceError::Rejected)?;
        let observation = Observation {
            id: report_key(edge_id, trip.now),
            observed_at: trip.now.to_string(),
            edge_id: edge_id.to_string(),
            kind,
            detail: detail.map(str::to_string),
            reporter_key: key.hex(),
        };
        let tiles = self.tiles.with_edge_mut(edge_id);
        if tiles.is_empty() {
            self.reports.record(
                std::slice::from_ref(&observation.edge_id),
                std::time::Duration::ZERO,
                Some(ErrorCode::UnknownEdge),
            );
            return Err(DeviceError::NoTile);
        }
        for tile in tiles {
            reports::apply_logged(
                &mut tile.graph,
                std::slice::from_ref(&observation),
                &mut self.reports,
            );
            self.router.forget_tile(&tile.id);
        }
        self.store
            .put(&observation.id, &reports::encode_observation(&observation))
            .map_err(|_| DeviceError::Rejected)?;
        Ok(observation)
    }

    /// Every sealed report, oldest record name first.
    pub fn sealed_reports(&self) -> Vec<Observation> {
        self.store
            .names_with_prefix("report:")
            .iter()
            .filter_map(|name| self.store.get(name).ok())
            .filter_map(|bytes| reports::decode_observation(&bytes).ok())
            .collect()
    }

    /// Applies the 90-day rule to the sealed reports. `age_days` gives each
    /// report's age. At 90 the detail is dropped and the report stays.
    /// Past 90 the sealed record is deleted.
    pub fn retain_reports(&mut self, age_days: impl Fn(&Observation) -> u32) {
        let aged: Vec<AgedReport> = self
            .sealed_reports()
            .into_iter()
            .map(|observation| AgedReport {
                age_days: age_days(&observation),
                observation,
            })
            .collect();
        for name in self.store.names_with_prefix("report:") {
            self.store.remove(&name);
        }
        for observation in reports::retain(aged) {
            let _ = self
                .store
                .put(&observation.id, &reports::encode_observation(&observation));
        }
    }

    pub fn training(&mut self) -> &mut TrainingRecord {
        &mut self.training
    }

    /// One pair per routed request while opted in: the chosen route against
    /// the best route that avoids its first edge, labelled by deterministic
    /// cost. A scratch router finds the alternative so the route cache and
    /// the log see nothing. No alternative, no pair.
    fn collect_pair(
        &mut self,
        graph: &Graph,
        request: &Request<'_>,
        route: &Route,
        chosen: Features,
    ) {
        let Some(first) = route.edge_ids.first() else {
            return;
        };
        let mut detour = graph.clone();
        for edge in &mut detour.edges {
            if edge.id == *first {
                edge.constraint = Constraint::Closed;
            }
        }
        let Some(alternative) = Router::new().route(&detour, request) else {
            return;
        };
        let Ok(other) = features(graph, &alternative.edge_ids) else {
            return;
        };
        let left_has_lower_cost =
            path_cost(graph, &route.edge_ids) <= path_cost(graph, &alternative.edge_ids);
        let _ = self.training.record(TrainingPair {
            left: chosen,
            right: other,
            left_has_lower_cost,
        });
    }

    /// Clears the sealed cache, the reporter key inside it, and the
    /// training opt-in. The cache key stays.
    pub fn clear_local_data(&mut self) {
        self.store.clear();
        self.forget_everything();
    }

    /// Rotates the cache key once `age_days` is at least 90. The sealed
    /// cache and the reporter key go with it. Earlier is refused and
    /// nothing changes.
    pub fn rotate_cache(&mut self, new_key: &[u8], age_days: u32) -> Result<(), StoreError> {
        self.store.rotate(new_key, age_days)?;
        self.forget_everything();
        Ok(())
    }

    /// What a clear or a rotation deletes beyond the sealed records: the
    /// tiles, the scorer artifacts, the active version, the budget, the
    /// cached routes, and the training record. The device routes again
    /// only after the platform reloads tiles and a scorer. The diagnostic
    /// log stays; it holds edge ids, durations, and error codes only.
    fn forget_everything(&mut self) {
        for id in self.budget.tile_ids() {
            self.router.forget_tile(&id);
        }
        self.tiles = TileCache::new();
        self.budget = Budget::with_limit(self.budget.limit());
        self.scorer.uninstall_all();
        self.active = None;
        self.training.clear_opt_in();
    }

    /// The reporter key from the sealed cache, created and sealed when the
    /// record is absent. A record that is present but does not open under
    /// this cache key is refused, not replaced. It is the only identifier
    /// a local report carries.
    pub fn reporter_key(&mut self) -> Result<ReporterKey, StoreError> {
        if self.store.export(reporter::RECORD).is_some() {
            let bytes = self.store.get(reporter::RECORD)?;
            return ReporterKey::from_bytes(&bytes).map_err(|_| StoreError::Rejected);
        }
        let key = ReporterKey::generate();
        self.store.put(reporter::RECORD, key.as_bytes())?;
        Ok(key)
    }

    /// A sealed record for the platform to persist. Still encrypted.
    pub fn export_record(&self, name: &str) -> Option<&[u8]> {
        self.store.export(name)
    }

    /// A sealed record the platform persisted earlier.
    pub fn import_record(&mut self, name: &str, sealed: Vec<u8>) {
        self.store.import(name, sealed);
    }

    pub fn tile_ids(&self) -> Vec<String> {
        self.budget.tile_ids()
    }

    pub fn budget_used(&self) -> u64 {
        self.budget.used()
    }

    /// Router lines, then scorer lines, then report lines. Edge ids,
    /// durations, and error codes only.
    pub fn diagnostics(&self) -> String {
        [
            self.router.diagnostics().render(),
            self.scorer.diagnostics().render(),
            self.reports.render(),
        ]
        .into_iter()
        .filter(|lines| !lines.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
    }

    fn resolve(
        &self,
        origin: &str,
        destination: Destination<'_>,
    ) -> Result<(&TileRecord, String), DeviceError> {
        match destination {
            Destination::NodeId(id) => {
                if id.is_empty() {
                    return Err(DeviceError::Rejected);
                }
                let tile = self
                    .tiles
                    .newest_covering(origin, id)
                    .ok_or(DeviceError::NoTile)?;
                Ok((tile, id.to_string()))
            }
            Destination::Coordinate { .. } => {
                let origin_tile = self
                    .tiles
                    .newest_covering(origin, origin)
                    .ok_or(DeviceError::NoTile)?;
                let snapped = snap::resolve(&origin_tile.graph, destination)
                    .map_err(|_| DeviceError::Rejected)?
                    .to_string();
                let tile = self
                    .tiles
                    .newest_covering(origin, &snapped)
                    .ok_or(DeviceError::NoTile)?;
                Ok((tile, snapped))
            }
        }
    }
}

fn route_key(origin: &str, destination: &str, tile_id: &str) -> String {
    format!("route:{origin}:{destination}:{tile_id}")
}

fn report_key(edge_id: &str, now: &str) -> String {
    format!("report:{edge_id}:{now}")
}

fn path_cost(graph: &Graph, edge_ids: &[String]) -> f64 {
    edge_ids
        .iter()
        .filter_map(|id| graph.edges.iter().find(|edge| edge.id == *id))
        .map(Edge::cost)
        .sum()
}

fn tile_key(id: &str) -> String {
    format!("tile:{id}")
}

fn model_key(id: &str) -> String {
    format!("model:{id}")
}

fn model_record_key(id: &str) -> String {
    format!("model-record:{id}")
}

const ACTIVE_RECORD: &str = "active-scorer";

/// Length-prefixed local record for the sealed cache. Not a contract.
fn encode_model_record(record: &ModelVersionRecord) -> Vec<u8> {
    let mut out = Vec::new();
    for field in [
        record.id.as_str(),
        record.artifact.as_str(),
        record.created_at.as_str(),
        record.previous_id.as_deref().unwrap_or(""),
    ] {
        out.extend((field.len() as u32).to_be_bytes());
        out.extend(field.as_bytes());
    }
    out
}

fn decode_model_record(bytes: &[u8]) -> Option<ModelVersionRecord> {
    let mut rest = bytes;
    let mut fields = Vec::with_capacity(4);
    for _ in 0..4 {
        let (len, after) = rest.split_at_checked(4)?;
        let len = u32::from_be_bytes(len.try_into().ok()?) as usize;
        let (field, after) = after.split_at_checked(len)?;
        fields.push(String::from_utf8(field.to_vec()).ok()?);
        rest = after;
    }
    if !rest.is_empty() {
        return None;
    }
    let previous_id = fields.pop()?;
    Some(ModelVersionRecord {
        previous_id: (!previous_id.is_empty()).then_some(previous_id),
        created_at: fields.pop()?,
        artifact: fields.pop()?,
        id: fields.pop()?,
    })
}
