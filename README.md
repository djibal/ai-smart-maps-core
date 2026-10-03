# AI-Smart-Maps core

Single-user routing core. It computes a route from a graph already in memory.
It does not contain a mesh stack, an account, or a network client.

Map adapters live beside the router and are not imported by it. The cost is `weight + 3 * hazard`. A missing hazard counts as 0.

The core boundary encodes graph, tile, report, confidence, model version, and route as protobuf. Those messages use the published field names and carry no personal data.
