# CHRE Architecture Specification

The Computational Hypergraph Rewrite Engine (CHRE) is a high-performance backend for executing topological graph grammars. The system is split across a Rust-based mathematical core and a Python-based orchestration layer.

This document details the exact technical architecture of the current codebase.

---

## 1. System Overview

CHRE operates on a strict **Orchestrator (Python) / Worker (Rust)** paradigm to maximize execution speed while maintaining ease of use.
* **The Python Layer (`chre_api.py`)**: Responsible for defining rules (Left-Hand Side / Right-Hand Side), managing configuration, determining execution chunk sizes (e.g., run 10,000 steps), and retrieving analytics.
* **The FFI Layer (`lib.rs`)**: Uses `PyO3` to serialize/deserialize data across the language boundary. Python calls cross this boundary exactly *once* per execution block.
* **The Core Engine (`src/*.rs`)**: Manages memory allocation, Subgraph Isomorphism matching, and graph mutations natively in Rust.

---

## 2. Core Memory Models (`src/hypergraph.rs`)

The `Hypergraph` struct manages the state of the universe. Unlike standard binary graphs, CHRE natively supports **Hyperedges** (edges that can connect $N$ vertices).

### `Vertex`
Every vertex is tracked using a globally unique `u64` ID via an atomic counter (`VERTEX_ID_COUNTER`).
* **Causal Tracking:** Each vertex possesses `parents` and `children` arrays, alongside highly optimized `FixedBitSet` structures (`causal_future` and `causal_past`) to track topological flow. 

### `Hypergraph` State
* **`vertices: HashMap<u64, Vertex>`**: The primary storage of node state.
* **`hyperedges: HashMap<u64, Hyperedge>`**: Stores edges, containing a `Vec<u64>` of participating vertices.
* **`vertex_to_edges: HashMap<u64, Vec<u64>>`**: An inverted index. Maps a vertex ID to all hyperedges containing it. This is mathematically critical for achieving $O(1)$ lookup times during Subgraph Isomorphism.

### Memory Configuration
* `initial_bitset_capacity`: Pre-allocates contiguous memory for bitsets to prevent `Vec` resizing operations as the graph grows.
* `causal_horizon`: A bounded limit for the Breadth-First Search (BFS) used when updating causal bitsets. Prevents $O(N)$ computational blowouts during dense graph interactions.

---

## 3. The Matcher (`src/grammar.rs`)

Graph matching is an NP-Complete problem. CHRE solves this using an optimized variant of the **VF2 Subgraph Isomorphism Algorithm**.

### The `MatchState`
During a search, the engine maintains a `MatchState` containing:
* `mapping`: A `HashMap<usize, u64>` connecting Abstract Rule IDs (e.g., node `0` in a rule) to Real Graph IDs (e.g., node `1045`).
* `used_real_vertices`: A `HashSet` ensuring injectivity (two abstract nodes cannot map to the same real node).

### Search Execution (`find_match`)
1. **Anchor Initialization:** The engine is provided a random real vertex (the "Anchor"). This is mapped to Abstract Node `0`.
2. **Recursive Backtracking:** The system calls `backtrack_search`. It attempts to map Abstract Node `n + 1` by searching the structural neighbors of already-mapped nodes.
3. **Constraint Validation:** For every tentative mapping, it checks if the fully mapped subsets of LHS edges actually exist in the real hypergraph using the `vertex_to_edges` inverted index.
4. **Backtrack:** If a constraint fails, the engine undoes the tentative mapping and tests the next neighbor.

---

## 4. The Executor (`src/grammar.rs`)

Once `find_match` yields a valid `MatchState`, the engine mutates the graph using the **Double-Pushout (DPO)** methodology.

### `apply_match`
1. **LHS Edge Deletion:** Identifies the exact real hyperedges corresponding to the matched LHS pattern and safely removes them.
2. **The Dangling Condition Enforcement:** 
   * The engine identifies abstract IDs present in the LHS but absent from the rule's `kept` list.
   * If a real vertex is marked for death, **all attached hyperedges are preemptively destroyed**. This satisfies the DPO Dangling Condition and prevents memory corruption/dangling pointers.
   * The vertex is then purged from the `Hypergraph`.
3. **RHS Vertex Spawning:** Iterates over the Right-Hand Side (RHS). If an abstract ID exists that is not currently mapped in `MatchState`, a new `Vertex` is initialized and injected into the graph.
4. **RHS Edge Generation:** The new topology is wired together. The inverted index (`vertex_to_edges`) is updated, and causal relations are linearly chained for the new structures.

---

## 5. The Engine Loop (`src/rewrite_engine.rs`)

The `RewriteEngine` struct orchestrates the execution lifecycle.

* **Anchor Selection:** To ensure mathematical uniform sampling without $O(N)$ overhead, `propose_rewrite` uses `SmallRng` to continuously guess random IDs between `0` and `max_vertex_id` until an active vertex is hit. This provides $O(1)$ amortized anchor selection.
* **Rule Selection:** A rule is selected uniformly from `grammar_rules`.
* **Execution:** Calls `find_match` -> `apply_match`.
* **State Updates:** Tracks `time` and `attempted_rewrites`.

---

## 6. PyO3 FFI Architecture (`src/lib.rs`)

To prevent FFI (Foreign Function Interface) overhead from stalling the engine, all structural mutations remain strictly in Rust.

* **Data Exposure:** Graph state is only serialized to Python when explicitly requested.
* Methods like `get_edges()` iterate over internal `HashMap` values and cast them into PyO3 compatible types (`Vec<Vec<u64>>`), which surface in Python as standard `List[List[int]]`.
* Structural queries (`get_shortest_path_distance`, `get_isolated_vertices`) execute fully in Rust (e.g., executing a native BFS) and only pass the final integer result back across the FFI boundary to Python.
