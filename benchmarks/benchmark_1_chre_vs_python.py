"""
benchmarks/benchmark_1_chre_vs_python.py

Benchmark 1: CHRE (Rust) vs a pure Python equivalent.

Both sides execute the same grammar:
  - Creation rule:   {} -> {A-B}
  - Expansion rule:  {A-B} -> {A-B, B-C, C-A}

The Python implementation uses plain dicts and lists — no external libraries.
This isolates the cost of Python's interpreter vs Rust's native execution.
"""
import time
import random
import sys
import os
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from chre_api import GraphUniverse, EngineConfig, Rule

STEPS = 50_000


# ---------------------------------------------------------------------------
# Pure Python graph grammar engine (same semantics as CHRE)
# ---------------------------------------------------------------------------

class PythonGraph:
    def __init__(self):
        self.vertices = set()
        self.edges = []          # list of frozensets
        self.next_id = 0

    def new_vertex(self):
        v = self.next_id
        self.next_id += 1
        self.vertices.add(v)
        return v

    def add_edge(self, *vs):
        self.edges.append(frozenset(vs))

    def neighbors(self, v):
        ns = set()
        for e in self.edges:
            if v in e:
                ns.update(e)
        ns.discard(v)
        return ns


def python_step(g: PythonGraph, rng: random.Random) -> bool:
    rule = rng.randint(0, 1)

    if rule == 0:
        # Creation rule: {} -> {A-B}
        a = g.new_vertex()
        b = g.new_vertex()
        g.add_edge(a, b)
        return True

    else:
        # Expansion rule: find {A-B}, keep A and B, add C and triangle edges
        if not g.edges:
            return False
        edge = rng.choice(g.edges)
        verts = list(edge)
        if len(verts) < 2:
            return False
        a, b = verts[0], verts[1]
        c = g.new_vertex()
        # Keep existing A-B edge, add B-C and C-A
        g.add_edge(b, c)
        g.add_edge(c, a)
        return True


def run_python(steps: int) -> tuple[int, float]:
    rng = random.Random(42)
    g = PythonGraph()
    t0 = time.perf_counter()
    for _ in range(steps):
        python_step(g, rng)
    elapsed = time.perf_counter() - t0
    return len(g.vertices), elapsed


def run_chre(steps: int) -> tuple[int, float]:
    u = GraphUniverse(EngineConfig(verbose=False, semantics="DPO"))
    u.add_rule(Rule(lhs=[], kept=[], rhs=[["A", "B"]]))
    u.add_rule(Rule(
        lhs=[["A", "B"]],
        kept=["A", "B"],
        rhs=[["A", "B"], ["B", "C"], ["C", "A"]]
    ))
    t0 = time.perf_counter()
    u.evolve(steps)
    elapsed = time.perf_counter() - t0
    return u.get_summary()["total_vertices"], elapsed


# ---------------------------------------------------------------------------
# Run
# ---------------------------------------------------------------------------

if __name__ == "__main__":
    print(f"Benchmark 1: CHRE (Rust) vs Pure Python")
    print(f"Grammar : creation rule + triangle expansion")
    print(f"Steps   : {STEPS:,}")
    print()

    py_verts, py_time = run_python(STEPS)
    chre_verts, chre_time = run_chre(STEPS)

    py_rate   = STEPS / py_time
    chre_rate = STEPS / chre_time
    speedup   = py_rate / chre_rate if py_rate > 0 else float("inf")

    print(f"{'':20s} {'Time (s)':>10s}   {'Steps/sec':>12s}   {'Vertices':>10s}")
    print(f"{'Pure Python':20s} {py_time:>10.4f}   {py_rate:>12,.0f}   {py_verts:>10,}")
    print(f"{'CHRE (Rust)':20s} {chre_time:>10.4f}   {chre_rate:>12,.0f}   {chre_verts:>10,}")
    print()

    if chre_rate > py_rate:
        print(f"  CHRE is {chre_rate/py_rate:.1f}x faster than the pure Python implementation.")
    else:
        print(f"  Pure Python is {py_rate/chre_rate:.1f}x faster than CHRE at this step count.")

    print()
    print("Note on interpretation:")
    print("  The Python implementation uses a simplified heuristic: it picks a random")
    print("  edge directly (O(1)) and applies the rule unconditionally. This is NOT")
    print("  a correct subgraph isomorphism — it skips pattern validation entirely.")
    print("  CHRE executes the full VF2-based matcher with DPO semantics on every step,")
    print("  which is algorithmically more expensive but mathematically correct.")
    print("  The comparison shows the interpreter overhead of Python, not the")
    print("  cost of the algorithm itself.")
