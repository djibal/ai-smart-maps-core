# AI-Smart-Maps core

Single-user routing core. It computes a route from a graph already in memory.
It does not contain a mesh stack, an account, or a network client.

Map adapters live beside the router and are not imported by it. The cost is `weight + 3 * hazard`. A missing hazard counts as 0.

The core boundary encodes graph, tile, report, confidence, model version, and route as protobuf. Those messages use the published field names and carry no personal data.

Local records are sealed with AES-256-GCM. The 32-byte key comes from the caller. Clearing the store deletes the sealed records.

The scorer runs the accepted ONNX bytes on four route features: edge count, weight sum, hazard sum, and missing-hazard count. A byte mismatch runs the previous artifact. The deterministic cost still chooses the route.

Diagnostic logs record edge ids, durations, and error codes. They do not record coordinates, reporter keys, or user ids.
