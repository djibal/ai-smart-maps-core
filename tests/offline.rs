use ai_smart_maps_core::confidence::{Environment, RouteNovelty};
use ai_smart_maps_core::graph::{Constraint, Edge, Graph, Node, Source};
use ai_smart_maps_core::router::{Request, Router};

fn graph() -> Graph {
    Graph {
        nodes: vec![
            Node {
                id: "a".to_string(),
                lat: 0.0,
                lon: 0.0,
            },
            Node {
                id: "c".to_string(),
                lat: 1.0,
                lon: 1.0,
            },
        ],
        edges: vec![Edge {
            id: "only".to_string(),
            src: "a".to_string(),
            dst: "c".to_string(),
            weight: 1.0,
            constraint: Constraint::Open,
            source: Source::Osm,
            hazard: None,
            valid_from: None,
            valid_to: None,
        }],
    }
}

#[test]
fn a_route_is_returned_with_no_network_client() {
    let ask = Request {
        origin: "a",
        destination: "c",
        tile_id: "tile-1",
        now: "2026-10-04T00:00:00Z",
        model_version_id: "scorer-1",
        map_age_days: 1.0,
        report_count: 0,
        rolled_back: false,
        route_novelty: RouteNovelty::Known,
        environment: Environment::Simple,
    };
    let route = Router::new().route(&graph(), &ask).expect("offline route");
    assert_eq!(route.edge_ids, vec!["only".to_string()]);

    let sources = [
        include_str!("../src/lib.rs"),
        include_str!("../src/router.rs"),
        include_str!("../src/graph.rs"),
        include_str!("../src/snap.rs"),
        include_str!("../src/confidence.rs"),
        include_str!("../src/device.rs"),
        include_str!("../src/tiles.rs"),
        include_str!("../src/reporter.rs"),
    ];
    for source in sources {
        assert!(!source.contains("reqwest"));
        assert!(!source.contains("TcpStream"));
        assert!(!source.contains("UdpSocket"));
        assert!(!source.contains("std::net"));
        assert!(!source.contains("hyper::"));
    }
}
