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
