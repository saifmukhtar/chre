"""
examples/02_semantics_comparison.py

Demonstrates the difference between DPO and SPO rewriting semantics.

Both universes run the same rules. Under DPO, a rewrite is aborted if deleting
a vertex would leave edges not covered by the LHS. Under SPO, those dangling
edges are deleted automatically. This produces different evolution rates.
"""
from chre_api import GraphUniverse, EngineConfig, Rule

STEPS = 20_000

rules = [
    Rule(lhs=[], kept=[], rhs=[["A", "B"]]),
    Rule(
        lhs=[["A", "B"]],
        kept=["A", "B"],
        rhs=[["A", "B"], ["B", "C"], ["C", "A"]]
    ),
]

print(f"Running {STEPS:,} steps under each semantics...\n")

for sem in ("DPO", "SPO"):
    u = GraphUniverse(EngineConfig(verbose=False, semantics=sem))
    for r in rules:
        u.add_rule(r)
    u.evolve(STEPS)
    s = u.get_summary()
    print(f"  {sem}  ->  vertices: {s['total_vertices']:>8,}   edges: {s['total_edges']:>8,}")

print()
print("Under DPO, rewrites that would create topological inconsistencies are aborted.")
print("Under SPO, those same rewrites proceed by auto-removing dangling edges.")
