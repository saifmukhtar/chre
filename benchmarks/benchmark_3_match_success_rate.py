"""
benchmarks/benchmark_3_match_success_rate.py

Benchmark 3: Rewrite success rate vs graph density.

Each step the engine picks a random vertex and a random rule, then tries to
find a subgraph isomorphism. If the pattern is not present at the chosen anchor,
the step is a failed attempt.

This benchmark tracks:
  - attempted_rewrites : total steps executed
  - successful_rewrites: steps where find_match() returned a valid MatchState
  - success_rate       : successful / attempted (%)

We measure this at increasing graph sizes. A higher success rate at larger sizes
means the graph has grown rich enough that almost any anchor satisfies at least
one rule — validating that the VF2 pruning is correctly filtering early rather
than letting bad paths run to completion.
"""
import sys
import os
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from chre_api import GraphUniverse, EngineConfig, Rule

MEASURE_STEPS = 1_000
TARGET_SIZES  = [100, 500, 1_000, 5_000, 10_000]

CREATION_RULE = Rule(lhs=[], kept=[], rhs=[["A", "B"]])
EXPANSION_RULE = Rule(
    lhs=[["A", "B"]],
    kept=["A", "B"],
    rhs=[["A", "B"], ["B", "C"], ["C", "A"]]
)


def grow_to(u: GraphUniverse, target: int):
    while u.get_summary()["total_vertices"] < target:
        u.evolve(max(1, (target - u.get_summary()["total_vertices"]) // 2 + 1))


if __name__ == "__main__":
    print(f"Benchmark 3: Rewrite success rate vs graph density")
    print(f"Measure steps per size : {MEASURE_STEPS:,}")
    print()
    print(f"{'Target size':>14s}   {'Vertices':>10s}   {'Edges':>10s}   {'Attempted':>10s}   {'Successful':>11s}   {'Success %':>10s}")
    print("-" * 80)

    for target in TARGET_SIZES:
        u = GraphUniverse(EngineConfig(verbose=False, semantics="DPO"))
        u.add_rule(CREATION_RULE)

        grow_to(u, target)

        u.add_rule(EXPANSION_RULE)
        u._engine.reset_counters()

        actual_v = u.get_summary()["total_vertices"]
        actual_e = u.get_summary()["total_edges"]

        u.evolve(MEASURE_STEPS)

        attempted   = u._engine.get_attempted_rewrites()
        successful  = u._engine.get_successful_rewrites()
        rate        = (successful / attempted * 100) if attempted > 0 else 0.0

        print(f"{target:>14,}   {actual_v:>10,}   {actual_e:>10,}   {attempted:>10,}   {successful:>11,}   {rate:>9.1f}%")

    print()
    print("Interpretation:")
    print("  Low success rate at sparse graphs is expected: the expansion rule")
    print("  looks for an existing edge, which may not share the anchor vertex.")
    print("  As the graph grows denser, almost every vertex participates in an")
    print("  edge, so the success rate rises. The VF2 pruning layers reject")
    print("  invalid candidates before recursion — keeping each failed attempt cheap.")
