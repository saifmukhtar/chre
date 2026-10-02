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

## How it works

CHRE is a rule-based topological engine. Each step:

1. **Anchor Selection:** A vertex is selected uniformly at random from the active vertex pool in $O(1)$ using a `swap_remove`-backed index.
2. **Matching:** The engine searches for a subgraph isomorphism between the rule's Left-Hand Side (LHS) and the real graph, using a VF2-based algorithm with T1/T2 frontier tracking and 1-lookahead and 2-lookahead feasibility pruning adapted for hyperedges.
3. **Execution:** If a match is found, the rule is applied under DPO semantics. The dangling condition is checked: if any vertex being deleted has edges not covered by the LHS match, the rewrite is aborted. Otherwise, LHS edges are deleted, dead vertices are removed, and RHS vertices and edges are created.

For full implementation details, see [ARCHITECTURE.md](ARCHITECTURE.md).

---

## Quickstart

```python
from chre_api import GraphUniverse, EngineConfig, Rule

config = EngineConfig(verbose=True, print_interval=10000)
universe = GraphUniverse(config)

# Spontaneous creation: empty LHS creates a new edge from nothing
universe.add_rule(Rule(lhs=[], kept=[], rhs=[["A", "B"]], weight=1.0))

# Triangle expansion: finds an edge and adds a new vertex connected to both endpoints
universe.add_rule(Rule(
    lhs=[["A", "B"]],
    kept=["A", "B"],
    rhs=[["A", "B"], ["B", "C"], ["C", "A"]],
    weight=1.0
))

universe.evolve(100_000)
print(universe.get_summary())
```

---

## Semantics toggle

The engine defaults to strict **DPO (Double-Pushout)** semantics. A rewrite is aborted if deleting a vertex would leave dangling edges not covered by the rule's LHS.

To switch to **SPO (Single-Pushout)** semantics — where dangling edges are automatically deleted along with the vertex — call:

```python
universe._engine.set_semantics("SPO")
```

---

## Configuration

```python
config = EngineConfig(
    verbose=True,
    print_interval=10000,    # Log progress every N steps
    bitset_capacity=1024,    # Initial FixedBitSet allocation for causal tracking
    causal_horizon=6,        # Max BFS depth for causal relation updates
)
```

---

## Analytics

```python
summary = universe.get_summary()
# {"total_vertices": ..., "total_edges": ..., "structural_pairs": ...}

raw = universe.get_raw_topology()
# {"vertices": [...], "edges": [[...], ...]}

dist = universe._engine.get_shortest_path_distance(node_start=0, node_target=45)
isolated = universe._engine.get_isolated_vertices()
edge_sizes = universe._engine.get_edge_size_map()
```

---

## License

Apache License 2.0
