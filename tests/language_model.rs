#[test]
fn the_routing_path_has_no_language_model() {
    let sources = [
        include_str!("../src/router.rs"),
        include_str!("../src/scorer.rs"),
        include_str!("../src/snap.rs"),
        include_str!("../src/graph.rs"),
        include_str!("../src/confidence.rs"),
        include_str!("../src/device.rs"),
        include_str!("../src/reporter.rs"),
    ];
    let forbidden = ["openai", "anthropic", "language model", "chatgpt", "gpt-"];
    for source in sources {
        let lower = source.to_ascii_lowercase();
        for needle in forbidden {
            assert!(!lower.contains(needle), "{needle}");
        }
    }
}
