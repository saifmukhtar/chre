# CHRE: Causal Hypergraph Rewrite Engine

[![PyPI version](https://badge.fury.io/py/chre.svg)](https://badge.fury.io/py/chre)
[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

**CHRE** is a blazing-fast, purely mathematical Graph Grammar execution engine. It allows you to define structural rewrite rules (topological grammars) and evolve complex hypergraphs over hundreds of thousands of steps in mere seconds.

Written entirely in **Rust** for maximum performance and exposed effortlessly to Python via **PyO3**, CHRE handles the NP-Complete problem of Subgraph Isomorphism (using an optimized VF2 algorithm) and applies Double-Pushout (DPO) graph rewriting at native speeds.

---

## 🚀 Installation

CHRE is distributed as pre-compiled Python wheels via PyPI. You do not need to install Rust or compile anything.

```bash
pip install chre
```

---

## 🧠 How it Works
CHRE is a strict, rule-based topological engine.
1. **Define a Rule:** Provide a Left-Hand Side (what shape to look for) and a Right-Hand Side (what to replace it with).
2. **Matcher:** The Rust engine rapidly scans the hypergraph for perfect topological isomorphisms.
3. **Executor:** The engine safely executes the rewrite, cleans up dangling edges, and spawns new vertices.

---

## 💻 Quickstart (Python)

You can orchestrate the entire universe directly from Python.

```python
import chre

# 1. Initialize the Engine
engine = chre.Engine()
engine.set_verbose(True)
engine.set_print_interval(50000)

# 2. Add Rules (Spontaneous Creation)
# LHS: [] -> Empty space
# RHS: [["A", "B"]] -> Creates two nodes connected by one edge
engine.add_rule(lhs=[], kept=[], rhs=[["A", "B"]], weight=1.0)

# 3. Add Rules (Triangle Expansion)
# LHS: [["A", "B"]] -> Finds an existing edge
# RHS: [["A", "B"], ["B", "C"], ["C", "A"]] -> Spawns node C and creates a triangle
engine.add_rule(
    lhs=[["A", "B"]], 
    kept=["A", "B"], 
    rhs=[["A", "B"], ["B", "C"], ["C", "A"]], 
    weight=1.0
)

# 4. Evolve the Universe!
print("Running 100,000 steps...")
engine.run(100_000)

# 5. Extract Analytics natively into Python
print(f"Total Nodes: {engine.node_count()}")
print(f"Total Edges: {engine.edge_count()}")

# Pull the raw graph data for NetworkX or visualization
edges = engine.get_edges()
degrees = engine.get_degree_map()
```

---

## ⚡ Advanced Configuration & Analytics

Because Python orchestration crosses the FFI boundary seamlessly, you can dynamically configure the engine and pull heavy topological analytics directly from the Rust backend.

```python
# Memory Pre-allocation (Optimizes RAM for massive graphs)
engine.set_bitset_capacity(1_000_000)

# Causal Horizon Limits (For directional flow mapping)
engine.set_causal_horizon(10)

# Topological Analytics (Calculated instantly in Rust)
isolated_nodes = engine.get_isolated_vertices()
edge_sizes = engine.get_edge_size_map()

# Calculate emergent geometric distance (Shortest Path)
dist = engine.get_shortest_path_distance(node_start=0, node_target=45)
```

---

## 📜 License
This project is licensed under the **Apache License 2.0**.
