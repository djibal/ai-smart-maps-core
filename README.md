# AI-Smart-Maps core

Single-user routing core. It computes a route from a graph already in memory.
It does not contain a mesh stack, an account, or a network client.

Map adapters live beside the router and are not imported by it. The cost is `weight + 3 * hazard`. A missing hazard counts as 0.

The core boundary encodes graph, tile, report, confidence, model version, and route as protobuf. Those messages use the published field names and carry no personal data.

Local records are sealed with AES-256-GCM. The 32-byte key comes from the caller. Clearing the store deletes the sealed records. At 90 days, `rotate` replaces that key and deletes the sealed records.

The scorer runs the accepted ONNX bytes on four route features: edge count, weight sum, hazard sum, and missing-hazard count. A byte mismatch runs the previous artifact. The deterministic cost still chooses the route.

Diagnostic logs record edge ids, durations, and error codes. They do not record coordinates, reporter keys, or user ids.

Maps and models share a 2GB budget. An insert that would pass it removes the oldest tile first. The active scorer and its `previous_id` artifact stay while rollback is still required.

A report keeps its detail before 90 days. At 90 days the detail is dropped. After 90 days the report is deleted.

Training pairs are stored only after an on-device opt-in. Clearing that opt-in deletes the pairs and the model version id.

A coordinate in range is snapped to the nearest node before the router is called. A node id that exists is kept. Any other destination is rejected. The router does not snap.

An initial route must finish within 2 seconds. A reroute must finish within 1 second. A slower result is not returned and is not cached.

A route is returned from the graph in memory when no network is available. The routing path has no network client and no language model.

Scorer inference must finish within 500 milliseconds. A slower inference is refused.

A local tile may omit a signature. A signature, when present, is Ed25519 over the unsigned tile protobuf and must verify with the 32-byte map-source public key. A fetched tile that fails that check leaves the cache unchanged.

The newest tile whose graph contains both node ids is the one used for those nodes.

`fixtures/a1v5_device_32x2.onnx` is the A1-v5 device scorer, 32 hidden units and 2 layers, trained on the linear variant with seed 0 and exported from the experiment repository. It runs on this host inside the 500 millisecond limit and ranks the cheaper route higher. The record beside it holds the reference scores. This is the accepted model shape running on device. It is not a new measurement of the 3-percentage-point bar.

`Device` runs the whole flow on one device: admit a tile, choose the newest tile that covers both nodes, snap, route, score, attach confidence, seal the route, and write the log. A tampered artifact rolls back to `previous_id` and lowers confidence. The deterministic cost decides the route.

`reporter` holds the reporter key: 16 random bytes from the operating system, written as 32 hex characters when a report carries it. `Device::reporter_key` creates and seals it on first use and reads it back after that. A sealed record that does not open under the current cache key is refused, not replaced. Clear and the 90-day rotation delete it with the cache. It is not in the log, the contracts, or the `Debug` output.

`Device::report` makes a local report: the observation carries the reporter key, is applied to every cached tile that has the edge, drops the cached routes on those tiles so the next request routes again, and is sealed under `report:{edge_id}:{now}`. An edge no tile knows is logged as `unknown_edge` and refused before sealing. `retain_reports` applies the 90-day rule to the sealed records and `clear_local_data` deletes them.

`Device::export_all` seals the whole cache into one blob, encrypted again under the cache key so record names stay private, and `import_all` rebuilds a device from it: tiles, scorer artifacts and their records, routes, reports, and the reporter key. A blob from another key is refused and nothing changes. The shell adapter exposes these as `save` and `restore`. `clear_local_data` and `rotate_cache` also drop the in-memory tiles, scorer artifacts, active version, budget, cached routes, and training record, so the device returns `NoScorer` or `NoTile` until the platform reloads. The diagnostic log stays.

While `training().opt_in` is set, every routed request records one training pair: the chosen route's four features against the best route that avoids its first edge, labelled by deterministic cost. A scratch router finds the alternative so the route cache and the log see nothing. No alternative, no pair. A repeated decision is kept once and the record holds at most `TRAINING_PAIR_CAP` (10,000) pairs, oldest out first. Nothing is recorded before opt-in and clearing deletes the pairs. `export_all` seals the training record, opt-in included, under `training`, and `import_all` restores it; a record that claims pairs without an opt-in is refused. Clear and rotation delete it.

`route` and `reroute` take `report_count` as the greater of the caller's number and the sealed reports on the device, so one sealed hazard is enough to raise the count on the route.

`install_scorer` drops every cached route so the next `route` on the same device runs on the new bytes, not a route that still names the previous model version.

`Device::reroute` continues a sealed route from a new position on the same tile, under the 1-second reroute limit. A tile whose `observed_at` is more than 90 days before `trip.now` lowers confidence. With no sealed route it returns `NoRoute`. Unreachable and rollback rules are the same as the initial path.

The iOS, Android, and web adapters call `Device`. No adapter calls the router or snaps on its own. Each one draws the returned edge list and stays under 10% of the core source. `show_reroute` continues a sealed trip from a new position under the same voice and tile limits; a closure on the remaining edge returns `Unreachable`, not the previous edge list.
