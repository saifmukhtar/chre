use fixedbitset::FixedBitSet;
use rand::Rng;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};

use smallvec::SmallVec;

static VERTEX_ID_COUNTER: AtomicU64 = AtomicU64::new(0);
static EDGE_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug)]
pub struct Vertex {
    pub id: u64,
    pub depth: usize, // Causal depth in the DAG
    pub state: SmallVec<[f64; 4]>, // AI Node Embedding / Feature Vector
    pub parents: Vec<u64>,
    pub children: Vec<u64>,
}

impl Vertex {
    pub fn new() -> Self {
        Self {
            id: VERTEX_ID_COUNTER.fetch_add(1, Ordering::SeqCst),
            depth: 1,
            state: SmallVec::new(),
            parents: Vec::new(),
            children: Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Hyperedge {
    pub id: u64,
    pub vertices: Vec<u64>,
}

impl Hyperedge {
    pub fn new(vertices: Vec<u64>) -> Self {
        Self {
            id: EDGE_ID_COUNTER.fetch_add(1, Ordering::SeqCst),
            vertices,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Hypergraph {
    pub vertices: HashMap<u64, Vertex>,
    pub hyperedges: HashMap<u64, Hyperedge>,
    pub causal_future: HashMap<u64, FixedBitSet>, // u.id -> J+(u)
    pub causal_past: HashMap<u64, FixedBitSet>,   // u.id -> J-(u)
    pub vertex_to_edges: HashMap<u64, Vec<u64>>, // v.id -> list of edge.ids
    pub active_edge_ids: Vec<u64>,               // For O(1) random selection
    pub max_depth: usize,                        // O(1) tracking
    pub structural_pair_count: usize,            // Incremental count of structural neighbor pairs
    pub causal_horizon: usize,                   // Stop BFS propagation after N hops (0 = infinite)
    pub initial_bitset_capacity: usize,          // Pre-allocation size for causal bitsets
}

impl Hypergraph {
    pub fn new() -> Self {
        Self {
            vertices: HashMap::new(),
            hyperedges: HashMap::new(),
            causal_future: HashMap::new(),
            causal_past: HashMap::new(),
            vertex_to_edges: HashMap::new(),
            active_edge_ids: Vec::new(),
            max_depth: 1,
            structural_pair_count: 0,
            causal_horizon: 6,
            initial_bitset_capacity: 1024,
        }
    }

    fn init_bitset(capacity: usize) -> FixedBitSet {
        FixedBitSet::with_capacity(capacity)
    }

    fn ensure_capacity(bs: &mut FixedBitSet, required_len: usize, min_cap: usize) {
        if bs.len() < required_len {
            let new_cap = required_len.max(min_cap).next_power_of_two();
            bs.grow(new_cap);
        }
    }

    // ---------- Vertex operations ----------

    pub fn add_vertex(&mut self) -> Vertex {
        let v = Vertex::new();

        let mut future = Self::init_bitset(self.initial_bitset_capacity);
        Self::ensure_capacity(&mut future, v.id as usize + 1, self.initial_bitset_capacity);
        future.insert(v.id as usize);
        self.causal_future.insert(v.id, future);

        let mut past = Self::init_bitset(self.initial_bitset_capacity);
        Self::ensure_capacity(&mut past, v.id as usize + 1, self.initial_bitset_capacity);
        past.insert(v.id as usize);
        self.causal_past.insert(v.id, past);

        self.vertices.insert(v.id, v.clone());
        v
    }

    // ---------- Hyperedge operations ----------

    pub fn max_vertex_id(&self) -> u64 {
        VERTEX_ID_COUNTER.load(Ordering::SeqCst)
    }

    pub fn add_hyperedge(&mut self, vertices: Vec<u64>) -> Hyperedge {
        let edge = Hyperedge::new(vertices.clone());
        self.hyperedges.insert(edge.id, edge.clone());
        self.active_edge_ids.push(edge.id);
        
        // Update index and interaction count
        for (i, &v1) in vertices.iter().enumerate() {
            self.vertex_to_edges
                .entry(v1)
                .or_insert_with(Vec::new)
                .push(edge.id);

            for &v2 in &vertices[i + 1..] {
                if !self.is_structurally_interacting_excluding(v1, v2, edge.id) {
                    self.structural_pair_count += 1;
                }
            }
        }

        edge
    }

    pub fn remove_hyperedge(&mut self, edge_id: u64) -> Option<Hyperedge> {
        let edge = self.hyperedges.remove(&edge_id)?;
        
        // Remove from active_edge_ids index (swap_remove is O(1))
        if let Some(pos) = self.active_edge_ids.iter().position(|&id| id == edge_id) {
            self.active_edge_ids.swap_remove(pos);
        }

        for (i, &v1) in edge.vertices.iter().enumerate() {
            if let Some(list) = self.vertex_to_edges.get_mut(&v1) {
                list.retain(|&id| id != edge_id);
            }

            for &v2 in &edge.vertices[i + 1..] {
                if !self.is_structurally_interacting(v1, v2) {
                    self.structural_pair_count = self.structural_pair_count.saturating_sub(1);
                }
            }
        }
        self.active_edge_ids.retain(|&id| id != edge_id);
        Some(edge)
    }

    pub fn edges_containing(&self, v_id: u64) -> Vec<u64> {
        self.vertex_to_edges.get(&v_id).cloned().unwrap_or_default()
    }

    // ---------- Causal structure ----------

    pub fn is_structurally_interacting(&self, u_id: u64, v_id: u64) -> bool {
        // Two vertices interact if they share at least one hyperedge.
        // This is O(k) where k is the coordination number (average ~10).
        if let Some(edges) = self.vertex_to_edges.get(&u_id) {
            for &e_id in edges {
                if let Some(edge) = self.hyperedges.get(&e_id) {
                    if edge.vertices.contains(&v_id) {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn is_structurally_interacting_excluding(
        &self,
        u_id: u64,
        v_id: u64,
        exclude_eid: u64,
    ) -> bool {
        if let Some(edges) = self.vertex_to_edges.get(&u_id) {
            for &e_id in edges {
                if e_id == exclude_eid {
                    continue;
                }
                if let Some(edge) = self.hyperedges.get(&e_id) {
                    if edge.vertices.contains(&v_id) {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn structural_neighbors(&self, v_id: u64) -> HashSet<u64> {
        let mut neighbors = HashSet::new();
        if let Some(edges) = self.vertex_to_edges.get(&v_id) {
            for &e_id in edges {
                if let Some(edge) = self.hyperedges.get(&e_id) {
                    for &n_id in &edge.vertices {
                        if n_id != v_id {
                            neighbors.insert(n_id);
                        }
                    }
                }
            }
        }
        neighbors
    }

    pub fn structural_neighbors_bitset(&self, v_id: &u64) -> Option<FixedBitSet> {
        let edges = self.vertex_to_edges.get(v_id)?;
        if edges.is_empty() {
            return None;
        }

        let mut bs = FixedBitSet::with_capacity(self.max_vertex_id() as usize + 1);
        for &e_id in edges {
            if let Some(edge) = self.hyperedges.get(&e_id) {
                for &n_id in &edge.vertices {
                    if n_id != *v_id {
                        let idx = n_id as usize;
                        if idx >= bs.len() {
                            bs.grow(idx + 1);
                        }
                        bs.insert(idx);
                    }
                }
            }
        }
        
        if bs.ones().next().is_none() {
            None
        } else {
            Some(bs)
        }
    }

    pub fn add_causal_relation(&mut self, u_id: u64, v_id: u64) {
        if self.is_causally_related(u_id, v_id) {
            return;
        }

        // Update v's depth based on u's depth (Axiom 3 precursor)
        let u_depth = self.vertices.get(&u_id).map(|u| u.depth).unwrap_or(1);
        if let Some(v) = self.vertices.get_mut(&v_id) {
            v.depth = v.depth.max(u_depth + 1);
        }

        // 1-hop adjacency (Guarded against duplicate entries)
        if let Some(u) = self.vertices.get_mut(&u_id) {
            if !u.children.contains(&v_id) {
                u.children.push(v_id);
            }
        }
        if let Some(v) = self.vertices.get_mut(&v_id) {
            if !v.parents.contains(&u_id) {
                v.parents.push(u_id);
            }
        }

        let v_depth = self.vertices.get(&v_id).map(|v| v.depth).unwrap_or(1);
        let horizon = if self.causal_horizon == 0 { usize::MAX } else { self.causal_horizon };

        // Pre-fetch bitsets for the update
        let cap = self.initial_bitset_capacity;
        let mut past_u = self.causal_past.get(&u_id).cloned().unwrap_or_else(|| Self::init_bitset(cap));
        let mut future_v = self.causal_future.get(&v_id).cloned().unwrap_or_else(|| Self::init_bitset(cap));

        let max_len = past_u.len().max(future_v.len());
        Self::ensure_capacity(&mut past_u, max_len, cap);
        Self::ensure_capacity(&mut future_v, max_len, cap);

        // 1. Update ancestors of U within horizon
        let mut queue = std::collections::VecDeque::new();
        let mut visited = FixedBitSet::with_capacity(self.max_vertex_id() as usize + 1);
        
        queue.push_back(u_id);
        visited.insert(u_id as usize);

        while let Some(curr_id) = queue.pop_front() {
            let curr_depth = self.vertices.get(&curr_id).map(|v| v.depth).unwrap_or(1);
            if u_depth.saturating_sub(curr_depth) > horizon {
                continue;
            }

            // Apply bitset update to ancestor
            if let Some(p_future) = self.causal_future.get_mut(&curr_id) {
                Self::ensure_capacity(p_future, max_len, cap);
                p_future.union_with(&future_v);
            }

            // Propagate further up
            if let Some(v) = self.vertices.get(&curr_id) {
                for &p_id in &v.parents {
                    if !visited.contains(p_id as usize) {
                        visited.insert(p_id as usize);
                        queue.push_back(p_id);
                    }
                }
            }
        }

        // 2. Update descendants of V within horizon
        queue.clear();
        visited.clear();
        queue.push_back(v_id);
        visited.insert(v_id as usize);

        while let Some(curr_id) = queue.pop_front() {
            let curr_depth = self.vertices.get(&curr_id).map(|v| v.depth).unwrap_or(1);
            if curr_depth.saturating_sub(v_depth) > horizon {
                continue;
            }

            // Apply bitset update to descendant
            if let Some(f_past) = self.causal_past.get_mut(&curr_id) {
                Self::ensure_capacity(f_past, max_len, cap);
                f_past.union_with(&past_u);
            }

            // Propagate further down
            if let Some(v) = self.vertices.get(&curr_id) {
                for &c_id in &v.children {
                    if !visited.contains(c_id as usize) {
                        visited.insert(c_id as usize);
                        queue.push_back(c_id);
                    }
                }
            }
        }
    }
    pub fn merge_causal_identity(&mut self, id_keep: u64, id_remove: u64) {
        let cap = self.initial_bitset_capacity;
        if let (Some(mut f_remove), Some(mut p_remove)) = (
            self.causal_future.get(&id_remove).cloned(),
            self.causal_past.get(&id_remove).cloned(),
        ) {
            if let Some(f_keep) = self.causal_future.get_mut(&id_keep) {
                let max_len = f_keep.len().max(f_remove.len());
                Self::ensure_capacity(f_keep, max_len, cap);
                Self::ensure_capacity(&mut f_remove, max_len, cap);
                f_keep.union_with(&f_remove);
            }
            if let Some(p_keep) = self.causal_past.get_mut(&id_keep) {
                let max_len = p_keep.len().max(p_remove.len());
                Self::ensure_capacity(p_keep, max_len, cap);
                Self::ensure_capacity(&mut p_remove, max_len, cap);
                p_keep.union_with(&p_remove);
            }
        }
    }

    pub fn is_causally_related(&self, u_id: u64, v_id: u64) -> bool {
        self.causal_future.get(&u_id).map_or(false, |bs| {
            if (v_id as usize) < bs.len() {
                bs.contains(v_id as usize)
            } else {
                false
            }
        })
    }

    pub fn remove_vertex(&mut self, v_id: u64) -> Option<Vertex> {
        let v = self.vertices.remove(&v_id)?;
        
        // Local scrubbing: remove v_id from causal sets of all vertices that could contain it.
        // We use the bitset directly to avoid HashSet allocations.
        if let Some(p_bs) = self.causal_past.get(&v_id) {
            let p_ids: Vec<usize> = p_bs.ones().collect();
            for p_id in p_ids {
                if let Some(f_bs) = self.causal_future.get_mut(&(p_id as u64)) {
                    if (v_id as usize) < f_bs.len() {
                        f_bs.set(v_id as usize, false);
                    }
                }
            }
        }

        if let Some(f_bs) = self.causal_future.get(&v_id) {
            let f_ids: Vec<usize> = f_bs.ones().collect();
            for f_id in f_ids {
                if let Some(p_bs) = self.causal_past.get_mut(&(f_id as u64)) {
                    if (v_id as usize) < p_bs.len() {
                        p_bs.set(v_id as usize, false);
                    }
                }
            }
        }

        self.causal_future.remove(&v_id);
        self.causal_past.remove(&v_id);
        self.vertex_to_edges.remove(&v_id);
        
        Some(v)
    }

    pub fn causal_future(&self, v_id: u64) -> impl Iterator<Item = u64> + '_ {
        self.causal_future
            .get(&v_id)
            .into_iter()
            .flat_map(|bs| bs.ones().map(|i| i as u64))
    }

    pub fn causal_past(&self, v_id: u64) -> impl Iterator<Item = u64> + '_ {
        self.causal_past
            .get(&v_id)
            .into_iter()
            .flat_map(|bs| bs.ones().map(|i| i as u64))
    }

    pub fn causal_future_bitset(&self, v_id: u64) -> Option<&FixedBitSet> {
        self.causal_future.get(&v_id)
    }

    pub fn causal_past_bitset(&self, v_id: u64) -> Option<&FixedBitSet> {
        self.causal_past.get(&v_id)
    }

    // ---------- Observables ----------

    pub fn coordination_number(&self, v_id: u64) -> usize {
        // O(1) lookup via index
        self.vertex_to_edges.get(&v_id).map(|edges| edges.len()).unwrap_or(0)
    }

    pub fn average_coordination(&self) -> f64 {
        if self.vertices.is_empty() {
            return 0.0;
        }
        let total_coordination: usize = self
            .vertices
            .keys()
            .map(|&v_id| self.coordination_number(v_id))
            .sum();
        total_coordination as f64 / self.vertices.len() as f64
    }

    // ---------- Worldline inertia ----------

    pub fn max_chain_length(&self) -> usize {
        self.max_depth
    }

    pub fn execute_undo_record(&mut self, record: crate::rules::UndoRecord) {
        // 1. Remove added vertices and their causal entries
        for v_id in &record.added_vertices {
            self.vertices.remove(v_id);
            self.causal_future.remove(v_id);
            self.causal_past.remove(v_id);
            self.vertex_to_edges.remove(v_id);
        }

        // 2. Remove added edges
        // Correct way to undo added edges in index:
        for eid in &record.added_edges {
            self.remove_hyperedge(*eid);
        }

        // 3. Restore removed vertices
        for (v_id, v) in record.removed_vertices {
            self.vertices.insert(v_id, v);
        }

        // 4. Restore removed edges
        for (eid, e) in record.removed_edges {
            self.hyperedges.insert(eid, e.clone());
            self.active_edge_ids.push(e.id);
            // Restore to index
            for &v in &e.vertices {
                self.vertex_to_edges.entry(v).or_insert_with(Vec::new).push(eid);
            }
        }

        // 5. Restore causal state and adjacency for modified vertices
        for (u_id, future_bs) in record.old_causal_future {
            self.causal_future.insert(u_id, future_bs);
        }
        for (u_id, past_bs) in record.old_causal_past {
            self.causal_past.insert(u_id, past_bs);
        }
        for (u_id, parents) in record.old_parents {
            if let Some(v) = self.vertices.get_mut(&u_id) {
                v.parents = parents;
            }
        }
        for (u_id, children) in record.old_children {
            if let Some(v) = self.vertices.get_mut(&u_id) {
                v.children = children;
            }
        }
    }
}
