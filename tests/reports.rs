use ai_smart_maps_core::graph::{Constraint, Edge, Graph, Node, Source};
use ai_smart_maps_core::reports::{apply, retain, AgedReport, Kind, Observation};

fn edge(id: &str) -> Edge {
    Edge {
        id: id.to_string(),
        src: "a".to_string(),
        dst: "b".to_string(),
        weight: 1.0,
        constraint: Constraint::Open,
        source: Source::Osm,
        hazard: None,
        valid_from: None,
        valid_to: None,
    }
}

fn graph() -> Graph {
    Graph {
        nodes: vec![Node {
            id: "a".to_string(),
            lat: 0.0,
            lon: 0.0,
        }],
        edges: vec![edge("e")],
    }
}

fn observation(kind: Kind, reporter: &str) -> Observation {
    Observation {
        id: format!("{reporter}-1"),
        observed_at: "2026-10-03T12:00:00Z".to_string(),
        edge_id: "e".to_string(),
        kind,
        detail: None,
        reporter_key: reporter.to_string(),
    }
}

#[test]
fn one_local_hazard_updates_the_edge_without_another_reporter() {
    let mut graph = graph();
    apply(&mut graph, &[observation(Kind::Hazard, "device")]);
    assert_eq!(graph.edges[0].hazard, Some(1.0));
    assert_eq!(graph.edges[0].constraint, Constraint::Open);
}

#[test]
fn one_local_closure_closes_the_edge() {
    let mut graph = graph();
    apply(&mut graph, &[observation(Kind::Closure, "device")]);
    assert_eq!(graph.edges[0].constraint, Constraint::Closed);
}

#[test]
fn one_local_clear_opens_the_edge_and_zeroes_hazard() {
    let mut graph = graph();
    graph.edges[0].constraint = Constraint::Closed;
    graph.edges[0].hazard = Some(0.8);
    apply(&mut graph, &[observation(Kind::Clear, "device")]);
    assert_eq!(graph.edges[0].constraint, Constraint::Open);
    assert_eq!(graph.edges[0].hazard, Some(0.0));
}

#[test]
fn an_equal_split_breaks_toward_closure() {
    let mut graph = graph();
    apply(
        &mut graph,
        &[
            observation(Kind::Hazard, "one"),
            observation(Kind::Closure, "two"),
        ],
    );
    assert_eq!(graph.edges[0].constraint, Constraint::Closed);
}

#[test]
fn a_hazard_majority_sets_hazard_to_the_reputation_share() {
    let mut graph = graph();
    apply(
        &mut graph,
        &[
            observation(Kind::Hazard, "one"),
            observation(Kind::Hazard, "two"),
            observation(Kind::Hazard, "three"),
            observation(Kind::Clear, "four"),
        ],
    );
    let hazard = graph.edges[0].hazard.expect("hazard");
    assert!(hazard > 0.5 && hazard < 1.0);
    assert_eq!(graph.edges[0].constraint, Constraint::Open);
}

#[test]
fn detail_drops_at_90_days_and_the_report_is_deleted_after_that() {
    let mut young = observation(Kind::Hazard, "young");
    young.detail = Some("lane blocked".to_string());
    let mut due = observation(Kind::Closure, "due");
    due.detail = Some("closed tonight".to_string());
    let mut old = observation(Kind::Clear, "old");
    old.detail = Some("clear again".to_string());

    let kept = retain(vec![
        AgedReport {
            observation: young,
            age_days: 89,
        },
        AgedReport {
            observation: due,
            age_days: 90,
        },
        AgedReport {
            observation: old,
            age_days: 91,
        },
    ]);

    assert_eq!(kept.len(), 2);
    assert_eq!(kept[0].detail.as_deref(), Some("lane blocked"));
    assert_eq!(kept[1].id, "due-1");
    assert_eq!(kept[1].detail, None);
    assert!(kept.iter().all(|report| report.id != "old-1"));
}
