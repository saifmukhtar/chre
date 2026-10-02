# CHRE: Causal Hypergraph Rewrite Engine

[![PyPI version](https://badge.fury.io/py/chre.svg)](https://badge.fury.io/py/chre)
[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

**CHRE** is a graph grammar execution engine written in Rust and exposed to Python via PyO3. It allows you to define structural rewrite rules over hypergraphs and evolve them over many steps at native speed.

The engine solves the Subgraph Isomorphism problem using a VF2-based algorithm adapted for hypergraphs, and applies rewrite rules under Double-Pushout (DPO) semantics with full dangling condition enforcement.

---

## When to use CHRE

If you need to analyze a static graph, use `NetworkX`.

CHRE is for cases where the **topology itself evolves** according to rules — where nodes and edges are created and destroyed based on structural pattern matching. Typical use cases include:

- Causal set models and discrete spacetime simulations (e.g., Wolfram Physics Project style)
- Graph-based cellular automata and artificial life
- Chemical reaction network simulation
- Any system where the interaction rules structurally modify the network

Standard Python graph libraries serialize and copy data for every mutation. CHRE keeps the entire graph in Rust memory and only crosses the Python/Rust boundary when you explicitly request data.

---

## Installation

CHRE is distributed as pre-compiled Python wheels via PyPI. No Rust toolchain is required.

```bash
pip install chre
```

---

## Features & Capabilities

- **Native Hypergraphs:** Supports generalized $N$-ary edges connecting any number of vertices.
- **VF2 Matching:** Uses all four Cordella feasibility layers (degree, arity, 1-lookahead, and 2-lookahead) to prune the search tree.
- **$O(1)$ Sampling:** True uniform random anchor selection using a `swap_remove`-backed contiguous index. No rejection sampling loops.
- **Named Observables:** Vertices carry physical state (e.g., `mass`, `spin`) directly in Rust memory.
- **Reversible Computing:** Built-in single-step execution and rollback API for branch-and-bound search.
- **Weighted Rules:** Rules are selected via discrete probability distributions built directly from user-defined weights.
- **DPO / SPO Semantics:** Toggle freely between Double-Pushout (strict graph matching) and Single-Pushout (automatic dangling edge cleanup).

---

## Quickstart

```python
from chre_api import GraphUniverse, EngineConfig, Rule

config = EngineConfig(verbose=True, print_interval=10000)
universe = GraphUniverse(config)

# 1. Spontaneous creation
universe.add_rule(Rule(lhs=[], kept=[], rhs=[["A", "B"]], weight=1.0))

# 2. Triangle expansion (Heavy weight so it fires more often)
universe.add_rule(Rule(
    lhs=[["A", "B"]],
    kept=["A", "B"],
    rhs=[["A", "B"], ["B", "C"], ["C", "A"]],
    weight=5.0
))

universe.evolve(100_000)
print(universe.get_summary())
```

---

## Named Observables (Physical State)

Vertices aren't just empty points; they can hold arbitrary physical state securely inside the engine.

```python
# Get a vertex ID from the graph
v_id = universe.get_raw_topology()["vertices"][0]

# Set and get physical variables
universe.set_observable(v_id, "mass", 1.5)
universe.set_observable(v_id, "spin", -0.5)

print(universe.get_observable(v_id, "mass"))  # 1.5

# Dump the entire universe's physical state mapping v_id -> {key: val}
all_state = universe.get_all_observables()
```

---

## Single-Step & Rollback (Reversible Search)

Instead of blindly running `evolve(n)`, you can run interactively. The engine internally generates `UndoRecord` structs that fully capture topological and causal mutations.

```python
# Try exactly one rewrite
fired = universe.step()

if fired:
    # If we didn't like what that rule did, we can undo it perfectly
    universe.rollback()
```

---

## Semantics Toggle

The engine defaults to strict **DPO (Double-Pushout)** semantics. A rewrite is aborted if deleting a vertex would leave dangling edges not covered by the rule's LHS.

To switch to **SPO (Single-Pushout)** semantics — where dangling edges are automatically deleted along with the vertex — call:

```python
universe.set_semantics("SPO")
print(universe.get_semantics())  # "SPO"
```

---

## Exporters

CHRE graphs can be exported natively to standard visualizers like **Gephi**, **Cytoscape**, or **NetworkX**.

```python
# Standard JSON format exactly mirroring internal memory
universe.save_json("simulation.json")

# Exports a bipartite graph (Vertices and Hyperedges both act as nodes)
# This perfectly preserves N-ary hyperedges and Named Observables in Gephi!
universe.save_graphml("simulation.graphml")
```

---

## License

Apache License 2.0
