# CHRE: Causal Hypergraph Rewrite Engine

[![PyPI version](https://badge.fury.io/py/chre.svg)](https://badge.fury.io/py/chre)
[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

**CHRE** is a blazing-fast, purely mathematical Graph Grammar execution engine. It allows you to define structural rewrite rules (topological grammars) and evolve complex hypergraphs over hundreds of thousands of steps in mere seconds.

Written entirely in **Rust** for maximum performance and exposed effortlessly to Python via **PyO3**, CHRE handles the NP-Complete problem of Subgraph Isomorphism (using an optimized VF2 algorithm) and applies Double-Pushout (DPO) graph rewriting at native speeds.

---

## 🎯 Is CHRE for you?

If you want to run graph analytics on a static network, use `NetworkX`. 
If you want to **dynamically evolve a network's topology** using rules, you need CHRE.

CHRE is specifically designed for researchers, data scientists, and physicists who need to simulate:
* **Emergent Geometries:** (e.g., Wolfram Physics Project models, Causal Set Theory).
* **Artificial Life & Cellular Automata:** Operating on dynamic graphs instead of rigid 2D grids.
* **Complex Systems:** Modeling social networks, chemical reaction networks, or distributed systems where the rules of interaction physically change the network's structure.

**Why CHRE?** Standard Python libraries choke when trying to dynamically add/remove thousands of nodes and edges per second while simultaneously solving subgraph isomorphisms. CHRE offloads 100% of the graph matching and memory allocation to Rust, while letting you orchestrate the rules easily in Python.

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

For a deep dive into how the engine is built, please read [ARCHITECTURE.md](ARCHITECTURE.md).

---

## 💻 Quickstart (Python)

You can orchestrate the entire universe directly from Python.

```python
from chre_api import GraphUniverse, EngineConfig, Rule

# 1. Initialize the Engine
config = EngineConfig(verbose=True, print_interval=10000)
universe = GraphUniverse(config)

# 2. Add Rules (Spontaneous Creation)
# LHS: [] -> Empty space
# RHS: [["A", "B"]] -> Creates two nodes connected by one edge
universe.add_rule(Rule(lhs=[], kept=[], rhs=[["A", "B"]], weight=1.0))

# 3. Add Rules (Triangle Expansion)
# LHS: [["A", "B"]] -> Finds an existing edge
# RHS: [["A", "B"], ["B", "C"], ["C", "A"]] -> Spawns node C and creates a triangle
universe.add_rule(Rule(
    lhs=[["A", "B"]], 
    kept=["A", "B"], 
    rhs=[["A", "B"], ["B", "C"], ["C", "A"]], 
    weight=1.0
))

# 4. Evolve the Universe!
print("Running 100,000 steps...")
universe.evolve(100_000)

# 5. Extract Analytics natively into Python
print(universe.get_summary())

# Pull the raw graph data for NetworkX or visualization
raw_data = universe.get_raw_topology()
edges = raw_data["edges"]
```

---

## ⚡ Advanced Configuration & Analytics

Because Python orchestration crosses the FFI boundary seamlessly, you can dynamically configure the engine and pull heavy topological analytics directly from the Rust backend.

```python
# Memory Pre-allocation (Optimizes RAM for massive graphs)
config.bitset_capacity = 1_000_000

# Causal Horizon Limits (For directional flow mapping)
config.causal_horizon = 10

# Topological Analytics (Calculated instantly in Rust)
isolated_nodes = universe._engine.get_isolated_vertices()
edge_sizes = universe._engine.get_edge_size_map()

# Calculate emergent geometric distance (Shortest Path)
dist = universe._engine.get_shortest_path_distance(node_start=0, node_target=45)
```

---

## 📜 License
This project is licensed under the **Apache License 2.0**.
