"""
examples/03_hyperedge_rules.py

Demonstrates hyperedge rules — rules where a single edge connects more than
two vertices. CHRE natively supports N-ary hyperedges.

This example creates 3-vertex hyperedges and rewrites them into two
overlapping 2-vertex edges, demonstrating hyperedge decomposition.
"""
from chre_api import GraphUniverse, EngineConfig, Rule

config = EngineConfig(verbose=True, print_interval=5000, semantics="DPO")
universe = GraphUniverse(config)

# Rule 1: Spontaneous creation of a 3-vertex hyperedge.
universe.add_rule(Rule(
    lhs=[],
    kept=[],
    rhs=[["A", "B", "C"]],   # One hyperedge connecting 3 nodes
    weight=1.0
))

# Rule 2: Decompose a 3-vertex hyperedge into two binary edges.
# Keeps A, B, C. Replaces the 3-ary hyperedge with two 2-ary edges.
universe.add_rule(Rule(
    lhs=[["A", "B", "C"]],
    kept=["A", "B", "C"],
    rhs=[["A", "B"], ["B", "C"]],
    weight=1.0
))

print("Running 10,000 steps with 3-vertex hyperedge decomposition...")
universe.evolve(10_000)

summary = universe.get_summary()
raw = universe.get_raw_topology()

print(f"\nFinal state:")
print(f"  Vertices : {summary['total_vertices']:,}")
print(f"  Edges    : {summary['total_edges']:,}")

# Show a few edge sizes to confirm hyperedges were created and decomposed
edge_sizes = {}
for edge in raw["edges"]:
    n = len(edge)
    edge_sizes[n] = edge_sizes.get(n, 0) + 1

print(f"\nEdge size distribution:")
for size, count in sorted(edge_sizes.items()):
    print(f"  {size}-vertex edges: {count:,}")
