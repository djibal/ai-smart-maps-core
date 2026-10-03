use ai_smart_maps_core::confidence::{Environment, RouteNovelty};
use ai_smart_maps_core::graph::{Constraint, Edge, Graph, Node, Source};
use ai_smart_maps_core::log::ErrorCode;
use ai_smart_maps_core::reports::{apply_logged, Kind, Observation};
use ai_smart_maps_core::router::{Request, Router};
use ai_smart_maps_core::scorer::{features, Host};

const COORDINATE: &str = "48.858844";
const REPORTER_KEY: &str = "rk-9f3c2a-not-a-log-field";

fn graph() -> Graph {
    Graph {
        nodes: vec![
            Node {
                id: "a".to_string(),
                lat: 48.858844,
                lon: 2.294351,
            },
            Node {
                id: "b".to_string(),
                lat: 48.860000,
                lon: 2.300000,
            },
        ],
        edges: vec![Edge {
            id: "east".to_string(),
            src: "a".to_string(),
            dst: "b".to_string(),
            weight: 2.0,
            constraint: Constraint::Open,
            source: Source::Osm,
            hazard: None,
            valid_from: None,
            valid_to: None,
        }],
    }
}

fn request<'a>(origin: &'a str, destination: &'a str) -> Request<'a> {
    Request {
        origin,
        destination,
        tile_id: "tile-1",
        now: "2026-10-04T00:00:00Z",
        model_version_id: "scorer-1",
        map_age_days: 1.0,
        report_count: 0,
        rolled_back: false,
        route_novelty: RouteNovelty::Known,
        environment: Environment::Simple,
    }
}

#[test]
fn a_route_log_keeps_edge_ids_and_drops_coordinates() {
    let mut router = Router::new();
    let route = router.route(&graph(), &request("a", "b")).expect("path");
    assert_eq!(route.edge_ids, vec!["east".to_string()]);

    let text = router.diagnostics().render();
    assert!(text.starts_with("edges=east duration_us="), "{text}");
    assert!(!text.contains("error="), "{text}");
    assert!(!text.contains(COORDINATE), "{text}");
    assert!(!text.contains("2.294351"), "{text}");
    assert!(!text.contains("scorer-1"), "{text}");
}

#[test]
fn an_unreachable_route_logs_the_error_code() {
    let mut router = Router::new();
    assert!(router.route(&graph(), &request("a", "missing")).is_none());
    let text = router.diagnostics().render();
    assert!(text.contains("edges= duration_us="), "{text}");
    assert!(text.contains("error=unreachable"), "{text}");
    assert!(!text.contains(COORDINATE), "{text}");
    assert_eq!(
        router.diagnostics().entries()[0].error_code,
        Some(ErrorCode::Unreachable)
    );
}

#[test]
fn a_refused_score_logs_the_error_and_not_the_presented_bytes() {
    let mut host = Host::new();
    host.install("scorer-1", b"good");
    let found = features(&graph(), &["east".to_string()]).unwrap();
    assert!(host
        .score("scorer-1", b"tampered-bytes", None, &found)
        .is_err());
    let text = host.diagnostics().render();
    assert!(text.contains("error=refused"), "{text}");
    assert!(text.contains("duration_us="), "{text}");
    assert!(!text.contains("tampered-bytes"), "{text}");
    assert!(!text.contains("scorer-1"), "{text}");
    assert!(!text.contains(COORDINATE), "{text}");
}

#[test]
fn a_report_log_keeps_the_edge_id_and_drops_the_reporter_key() {
    let mut graph = graph();
    let mut log = ai_smart_maps_core::log::DiagnosticLog::new();
    apply_logged(
        &mut graph,
        &[
            Observation {
                id: "obs-1".to_string(),
                observed_at: "2026-10-04T00:00:00Z".to_string(),
                edge_id: "east".to_string(),
                kind: Kind::Hazard,
                detail: Some(format!("met the user at {COORDINATE}")),
                reporter_key: REPORTER_KEY.to_string(),
            },
            Observation {
                id: "obs-2".to_string(),
                observed_at: "2026-10-04T00:00:01Z".to_string(),
                edge_id: "missing-edge".to_string(),
                kind: Kind::Closure,
                detail: None,
                reporter_key: REPORTER_KEY.to_string(),
            },
        ],
        &mut log,
    );
    let text = log.render();
    assert!(text.contains("edges=east duration_us="), "{text}");
    assert!(text.contains("edges=missing-edge duration_us="), "{text}");
    assert!(text.contains("error=unknown_edge"), "{text}");
    assert!(!text.contains(REPORTER_KEY), "{text}");
    assert!(!text.contains(COORDINATE), "{text}");
    assert!(!text.contains("obs-1"), "{text}");
}
