# CHRE Architecture Specification

**CHRE** (Causal Hypergraph Rewrite Engine) is a Rust-based execution engine for topological graph grammars. This document describes the exact implementation of the current codebase.

---

## 1. System Overview

CHRE separates concerns across three layers:

- **Python layer (`chre_api.py`):** Rule definition, configuration, execution scheduling, and analytics retrieval.
- **FFI layer (`src/lib.rs`):** PyO3 bindings. Python calls cross this boundary once per `evolve(n)` block, not once per step.
- **Core layer (`src/*.rs`):** All graph memory, matching, and mutation logic runs entirely in Rust.

---

## 2. Memory Model (`src/hypergraph.rs`)

The `Hypergraph` struct is the primary state container.

### `Vertex`
Each vertex has a globally unique `u64` ID assigned by an atomic counter (`VERTEX_ID_COUNTER`). Vertices carry:
- `parents: Vec<u64>` and `children: Vec<u64>` for direct causal relations.
- `causal_past: FixedBitSet` and `causal_future: FixedBitSet` for transitive closure under BFS, bounded by `causal_horizon`.

### `Hyperedge`
An edge connecting an ordered `Vec<u64>` of vertex IDs. Unlike binary edges, a single hyperedge can connect $N$ vertices.

### `Hypergraph` fields
| Field | Type | Purpose |
|---|---|---|
| `vertices` | `HashMap<u64, Vertex>` | Primary vertex storage |
| `hyperedges` | `HashMap<u64, Hyperedge>` | Primary edge storage |
| `vertex_to_edges` | `HashMap<u64, Vec<u64>>` | Inverted index: vertex → incident edge IDs |
| `active_vertex_ids` | `Vec<u64>` | Contiguous pool of live vertex IDs for $O(1)$ random sampling |
| `vertex_to_active_index` | `HashMap<u64, usize>` | Maps vertex ID to its position in `active_vertex_ids` |

### $O(1)$ vertex removal
When a vertex is deleted, `vertex_to_active_index` gives its position in `active_vertex_ids` in $O(1)$. A `swap_remove` replaces it with the last element, and the index of the swapped element is updated. This maintains a dense pool without gaps at all times.

### Memory configuration
- `initial_bitset_capacity`: Pre-allocates `FixedBitSet` memory to reduce allocator pressure as the graph grows.
- `causal_horizon`: Bounds BFS depth when propagating causal relations. Prevents $O(N)$ traversals on dense graphs.

---

## 3. The Matcher (`src/grammar.rs`)

Subgraph isomorphism is NP-Complete in the general case. CHRE uses a **VF2-based algorithm adapted for hypergraphs**, implementing the full four-layer feasibility hierarchy from Cordella et al. (2001).

### `RewriteRule` pre-computation
When a rule is constructed, the following are computed once and stored:

- `lhs_edges: Vec<Vec<usize>>` — the abstract LHS topology using integer IDs instead of string names.
- `kept_vertices: HashSet<usize>` — abstract IDs that survive the rewrite.
- `total_lhs_nodes: usize` — the number of unique nodes in the LHS only (RHS-only nodes are excluded from matching).
- `lhs_node_degrees: HashMap<usize, usize>` — the hyperedge degree of each abstract LHS node, pre-computed for $O(1)$ feasibility checks.

### `MatchState`
The search state carried through recursion:

| Field | Type | Purpose |
|---|---|---|
| `mapping` | `HashMap<usize, u64>` | Abstract node ID → real vertex ID |
| `used_real_vertices` | `HashSet<u64>` | Injectivity guard |
| `t2_frontier` | `HashSet<u64>` | Unmapped real vertices adjacent to the current mapping |
| `t1_frontier` | `HashSet<usize>` | Unmapped abstract nodes adjacent to the current mapping |

### `find_match`
1. Abstract node `0` is forced to the anchor vertex. The `t1_frontier` and `t2_frontier` are seeded from the anchor's neighborhood.
2. `backtrack_search` is called starting at abstract node `1`.

### `backtrack_search` — Feasibility layers

At each step, before committing a candidate mapping $(n, m)$ where $n$ is an abstract node and $m$ is a real vertex:

**Layer 0 — Degree pruning:**
The real vertex must have at least as many incident hyperedges as the abstract node requires.
$$|\text{edges}(m)| \geq \text{degree}(n)$$
This is an $O(1)$ lookup using `lhs_node_degrees`.

**Layer 1 — Arity pruning:**
For any fully-mapped LHS edge, the real edge it maps to must contain at least as many vertices as the abstract edge.
$$|\text{vertices}(\text{real\_edge})| \geq |\text{vertices}(\text{abstract\_edge})|$$
Checked during constraint validation.

**Layer 2 — 1-Lookahead (T frontier feasibility):**
The number of abstract neighbors of $n$ that are in $T_1$ (reachable frontier) must not exceed the number of real neighbors of $m$ that are in $T_2$.
$$|T_2 \cap \text{neighbors}(m)| \geq |T_1 \cap \text{unmapped\_neighbors}(n)|$$

**Layer 3 — 2-Lookahead ($\tilde{N}$ outside-frontier feasibility):**
The number of abstract neighbors of $n$ outside $T_1$ and outside the mapping ($\tilde{N}_1$) must not exceed the real neighbors of $m$ outside $T_2$ and outside the mapping ($\tilde{N}_2$).
$$|\tilde{N}_2 \cap \text{neighbors}(m)| \geq |\tilde{N}_1 \cap \text{unmapped\_neighbors}(n)|$$

### Frontier maintenance
After a successful tentative mapping, `t2_delta` and `t1_delta` record exactly which vertices were added to each frontier. On backtrack, those additions are reversed and the unmapped candidate is restored to $T_2$. This ensures the frontier state is always consistent with the current mapping depth.

### Candidate generation
If the current abstract node is in $T_1$, candidates are drawn only from $T_2$. This constrains the search to the structurally reachable neighborhood, rather than the full vertex set. The fallback to all vertices applies only when the LHS has disconnected components.

---

## 4. The Executor (`src/grammar.rs`)

Once `find_match` yields a valid `MatchState`, `apply_match` mutates the graph under **Double-Pushout (DPO) semantics**.

### `apply_match` steps

1. **LHS edge deletion:** For each abstract LHS edge, the corresponding real hyperedge is identified and removed via `remove_hyperedge`.

2. **DPO dangling condition enforcement:**  
   For each abstract node that is *not* in `kept_vertices` (i.e., it is deleted by the rule), the engine collects all hyperedges incident to its real mapping. In DPO mode (`strict_dpo = true`), if any such edge is not covered by the LHS match, the rewrite is aborted. In SPO mode, those edges are deleted automatically.  
   After edge cleanup, the vertex is removed via `remove_vertex`, which runs the $O(1)$ `swap_remove` against `active_vertex_ids`.

3. **Causal state backup:** Before deleting a vertex, causal bitsets of the vertex and its neighborhood are saved to the `UndoRecord` for potential rollback.

4. **RHS vertex creation:** Abstract IDs in the RHS that are not yet in `mapping` are instantiated as new vertices via `add_vertex`, which inserts them into `active_vertex_ids`.

5. **RHS edge creation:** New hyperedges are registered, the `vertex_to_edges` index is updated, and causal relations are linearly chained along the new edge.

---

## 5. Engine Loop (`src/rewrite_engine.rs`)

`RewriteEngine` manages the per-step execution cycle.

### Anchor selection
A live vertex is sampled uniformly in $O(1)$ by calling `active_vertex_ids.choose(rng)`. Because `active_vertex_ids` is maintained as a dense, gap-free pool by the `swap_remove` strategy, every entry is guaranteed to be an active vertex. There is no rejection sampling.

### Step execution
```
active_vertex_ids.choose(rng)  →  anchor
grammar_rules.choose(rng)      →  rule
rule.find_match(graph, anchor, strict_dpo)  →  Option<MatchState>
rule.apply_match(graph, match_state)        →  UndoRecord
```

### Configuration
| Field | Default | Effect |
|---|---|---|
| `strict_dpo` | `true` | Enforces DPO dangling condition. Set to `false` for SPO. |
| `verbose` | `false` | Enables per-interval progress logging. |
| `print_interval` | `10000` | Steps between log lines. |

---

## 6. FFI Layer (`src/lib.rs`)

PyO3 wraps the engine in a `#[pyclass]` struct. All graph computation stays in Rust. Python only sends:
- Rule definitions (string names, which are converted to integer IDs at construction time)
- Configuration values
- Step counts

Python only receives:
- Aggregated summary dicts
- Raw topology data (serialized on explicit request)
- Scalar query results (path distances, isolated vertex lists)

The FFI boundary is crossed once per `evolve(n)` call, not once per step.
