"""
benchmarks/benchmark_2_scaling.py

Benchmark 2: CHRE throughput at increasing graph sizes.

Methodology:
  1. Pre-grow the graph to a target vertex count using the creation rule.
  2. Reset the step counters (do not reset the graph).
  3. Run a fixed number of measurement steps with both rules active.
  4. Record steps/second at each size.

This shows how the VF2 matcher and DPO executor scale as graph density grows.
"""
import time
import sys
import os
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from chre_api import GraphUniverse, EngineConfig, Rule

MEASURE_STEPS   = 2_000     # Steps to time at each size (short enough to not OOM)
TARGET_SIZES    = [500, 2_000, 5_000, 10_000, 20_000]

CREATION_RULE = Rule(lhs=[], kept=[], rhs=[["A", "B"]])
EXPANSION_RULE = Rule(
    lhs=[["A", "B"]],
    kept=["A", "B"],
    rhs=[["A", "B"], ["B", "C"], ["C", "A"]]
)


def grow_to(u: GraphUniverse, target: int):
    """Grow graph to at least target vertices using only creation rule."""
    while u.get_summary()["total_vertices"] < target:
        u.evolve(max(1, (target - u.get_summary()["total_vertices"]) // 2 + 1))


if __name__ == "__main__":
    print(f"Benchmark 2: CHRE throughput vs graph size")
    print(f"Measure steps per size : {MEASURE_STEPS:,}")
    print()
    print(f"{'Target size':>14s}   {'Actual vertices':>16s}   {'Edges':>10s}   {'Steps/sec':>12s}   {'ms/step':>10s}")
    print("-" * 75)

    for target in TARGET_SIZES:
        u = GraphUniverse(EngineConfig(verbose=False, semantics="DPO"))
        u.add_rule(CREATION_RULE)

        # Phase 1: grow to target size
        grow_to(u, target)

        # Phase 2: add expansion rule and reset counters
        u.add_rule(EXPANSION_RULE)
        u._engine.reset_counters()
        actual_v = u.get_summary()["total_vertices"]
        actual_e = u.get_summary()["total_edges"]

        # Phase 3: measure
        t0 = time.perf_counter()
        u.evolve(MEASURE_STEPS)
        elapsed = time.perf_counter() - t0

        rate   = MEASURE_STEPS / elapsed
        ms_per = (elapsed / MEASURE_STEPS) * 1000

        print(f"{target:>14,}   {actual_v:>16,}   {actual_e:>10,}   {rate:>12,.0f}   {ms_per:>10.4f}")

    print()
    print("Note: throughput decreases as the graph grows because the VF2 matcher")
    print("must search a larger neighbourhood. The rate of decrease reflects how")
    print("well the frontier-guided pruning contains the search space.")
