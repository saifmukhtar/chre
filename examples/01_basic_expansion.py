"""
examples/01_basic_expansion.py

Demonstrates the minimal working simulation:
- A spontaneous creation rule generates an initial edge.
- A triangle expansion rule grows the graph from that edge.
"""
from chre_api import GraphUniverse, EngineConfig, Rule

config = EngineConfig(
    verbose=True,
    print_interval=10000,
    semantics="DPO",
)
universe = GraphUniverse(config)

# Rule 1: Spontaneous creation. Empty LHS means this fires unconditionally.
# It produces two connected nodes from nothing.
universe.add_rule(Rule(
    lhs=[],
    kept=[],
    rhs=[["A", "B"]],
    weight=1.0
))

# Rule 2: Triangle expansion. Finds any edge [A,B], keeps both endpoints,
# introduces a new node C, and wires a triangle.
universe.add_rule(Rule(
    lhs=[["A", "B"]],
    kept=["A", "B"],
    rhs=[["A", "B"], ["B", "C"], ["C", "A"]],
    weight=1.0
))

print("Running 50,000 steps with triangle expansion...")
universe.evolve(50_000)

summary = universe.get_summary()
print(f"\nFinal state:")
print(f"  Vertices : {summary['total_vertices']:,}")
print(f"  Edges    : {summary['total_edges']:,}")
