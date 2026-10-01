use std::collections::{HashMap, HashSet};
use crate::hypergraph::{Hypergraph, Vertex};
use crate::rules::UndoRecord;

#[derive(Clone, Debug)]
pub struct RewriteRule {
    /// Abstract representation of the Left-Hand Side hyperedges (e.g., [[0, 1], [2, 3]])
    pub lhs_edges: Vec<Vec<usize>>,
    
    /// Abstract vertices that must NOT be deleted (e.g., {0, 1, 2, 3})
    pub kept_vertices: HashSet<usize>,
    
    /// Abstract representation of the Right-Hand Side hyperedges
    pub rhs_edges: Vec<Vec<usize>>,
    
    /// Probability/Weight of this rule being selected
    pub weight: f64,
    
    /// Maps the user's string names ("A", "B") to our internal fast indices (0, 1)
    pub name_map: HashMap<String, usize>,
}

impl RewriteRule {
    pub fn new(
        lhs_strs: Vec<Vec<String>>,
        kept_strs: Vec<String>,
        rhs_strs: Vec<Vec<String>>,
        weight: f64,
    ) -> Self {
        let mut name_map = HashMap::new();
        let mut next_id = 0;

        // Helper to map string variable names to fast internal integer IDs
        let get_or_assign_id = |name: &String, map: &mut HashMap<String, usize>, id_counter: &mut usize| -> usize {
            *map.entry(name.clone()).or_insert_with(|| {
                let id = *id_counter;
                *id_counter += 1;
                id
            })
        };

        let mut lhs_edges = Vec::new();
        for edge in lhs_strs {
            let mapped_edge = edge.iter().map(|n| get_or_assign_id(n, &mut name_map, &mut next_id)).collect();
            lhs_edges.push(mapped_edge);
        }

        let mut rhs_edges = Vec::new();
        for edge in rhs_strs {
            let mapped_edge = edge.iter().map(|n| get_or_assign_id(n, &mut name_map, &mut next_id)).collect();
            rhs_edges.push(mapped_edge);
        }

        let mut kept_vertices = HashSet::new();
        for name in kept_strs {
            if let Some(&id) = name_map.get(&name) {
                kept_vertices.insert(id);
            }
        }

        Self {
            lhs_edges,
            kept_vertices,
            rhs_edges,
            weight,
            name_map,
        }
    }

    /// Attempts to find a true topological match for the LHS in the real hypergraph.
    pub fn find_match(&self, graph: &Hypergraph, anchor_real: u64) -> Option<MatchState> {
        // If the rule has no LHS, it's a spontaneous creation rule. Match instantly.
        if self.lhs_edges.is_empty() {
            return Some(MatchState::new());
        }

        let mut state = MatchState::new();
        
        // Anchor the search: Force Abstract Node 0 to be the real vertex the engine gave us.
        state.mapping.insert(0, anchor_real);
        state.used_real_vertices.insert(anchor_real);

        let total_abstract_nodes = self.name_map.len();

        // Start the heavy recursive backtracking algorithm at abstract node 1
        if self.backtrack_search(1, total_abstract_nodes, &mut state, graph) {
            Some(state)
        } else {
            None
        }
    }

    /// The core recursive Subgraph Isomorphism algorithm (VF2-style)
    fn backtrack_search(
        &self,
        current_abstract_id: usize,
        total_abstract_nodes: usize,
        state: &mut MatchState,
        graph: &Hypergraph,
    ) -> bool {
        // 1. BASE CASE: All abstract nodes have been perfectly mapped!
        if current_abstract_id == total_abstract_nodes {
            return true;
        }

        // 2. CANDIDATE GENERATION: Look for an already-mapped neighbor.
        let mut candidates: HashSet<u64> = HashSet::new();
        let mut found_anchor = false;

        for edge in &self.lhs_edges {
            if edge.contains(&current_abstract_id) {
                // Find an abstract node in this edge that is ALREADY mapped (id < current_abstract_id)
                for &other_abstract in edge {
                    if other_abstract < current_abstract_id {
                        if let Some(&real_id) = state.mapping.get(&other_abstract) {
                            found_anchor = true;
                            // Add all real structural neighbors of this mapped node to our candidates
                            if let Some(edge_ids) = graph.vertex_to_edges.get(&real_id) {
                                for &e_id in edge_ids {
                                    if let Some(real_edge) = graph.hyperedges.get(&e_id) {
                                        for &neighbor_v in &real_edge.vertices {
                                            candidates.insert(neighbor_v);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // If the rule has disconnected components (rare), we fall back to searching all vertices.
        let candidate_list: Vec<u64> = if found_anchor {
            candidates.into_iter().collect()
        } else {
            graph.vertices.keys().copied().collect()
        };

        // 3. THE BACKTRACKING LOOP: Try every candidate
        for real_candidate in candidate_list {
            // INJECTIVITY: We cannot map two abstract IDs to the same real vertex
            if state.used_real_vertices.contains(&real_candidate) {
                continue;
            }

            // TENTATIVE GUESS: Map the candidate
            state.mapping.insert(current_abstract_id, real_candidate);
            state.used_real_vertices.insert(real_candidate);

            // 4. VALIDITY CHECK: Do all fully-mapped LHS edges exist in the real graph?
            let mut is_valid = true;
            for lhs_edge in &self.lhs_edges {
                let is_fully_mapped = lhs_edge.iter().all(|a_id| state.mapping.contains_key(a_id));
                
                if is_fully_mapped {
                    let real_vertices: Vec<u64> = lhs_edge.iter().map(|a_id| *state.mapping.get(a_id).unwrap()).collect();
                    
                    let mut shared_edge_exists = false;
                    if let Some(&first_v) = real_vertices.first() {
                        if let Some(edge_ids) = graph.vertex_to_edges.get(&first_v) {
                            for &e_id in edge_ids {
                                if let Some(real_edge) = graph.hyperedges.get(&e_id) {
                                    // Verify that THIS real hyperedge contains ALL the required vertices
                                    if real_vertices.iter().all(|v| real_edge.vertices.contains(v)) {
                                        shared_edge_exists = true;
                                        break;
                                    }
                                }
                            }
                        }
                    }

                    if !shared_edge_exists {
                        is_valid = false;
                        break; // Fail early, this mapping is invalid
                    }
                }
            }

            // 5. RECURSION: If the guess is valid so far, dig deeper!
            if is_valid {
                if self.backtrack_search(current_abstract_id + 1, total_abstract_nodes, state, graph) {
                    return true; // The entire branch succeeded!
                }
            }

            // 6. BACKTRACK: The guess failed deeper down. Undo it and try the next candidate.
            state.mapping.remove(&current_abstract_id);
            state.used_real_vertices.remove(&real_candidate);
        }

        // If we exhaust all candidates and none work, this branch is dead.
        false
    }

    /// The Executor: Applies the rule to the graph using the Double-Pushout (DPO) method.
    pub fn apply_match(&self, h: &mut Hypergraph, mut match_state: MatchState) -> UndoRecord {
        let mut undo = UndoRecord::default();
        
        // 1. DELETE LHS EDGES
        // For every edge in LHS, find the exact matching hyperedge in the real graph and remove it.
        for lhs_edge in &self.lhs_edges {
            let real_vertices: Vec<u64> = lhs_edge.iter().map(|a_id| *match_state.mapping.get(a_id).unwrap()).collect();
            let mut edge_to_remove = None;
            
            if let Some(&first_v) = real_vertices.first() {
                if let Some(edge_ids) = h.vertex_to_edges.get(&first_v) {
                    for &e_id in edge_ids {
                        if let Some(real_edge) = h.hyperedges.get(&e_id) {
                            // Exact match: Same length and contains all vertices
                            if real_edge.vertices.len() == real_vertices.len() && real_vertices.iter().all(|v| real_edge.vertices.contains(v)) {
                                edge_to_remove = Some(e_id);
                                break;
                            }
                        }
                    }
                }
            }
            
            if let Some(e_id) = edge_to_remove {
                if let Some(e) = h.remove_hyperedge(e_id) {
                    undo.removed_edges.insert(e_id, e);
                }
            }
        }

        // 2. SCRUB DEAD VERTICES (Dangling Condition)
        // Find abstract IDs in the rule that are NOT in `kept_vertices`
        let all_abstract_ids: HashSet<usize> = self.name_map.values().copied().collect();
        for abstract_id in all_abstract_ids {
            // Is it a node we matched from LHS?
            if let Some(&real_id) = match_state.mapping.get(&abstract_id) {
                if !self.kept_vertices.contains(&abstract_id) {
                    // DPO Dangling Condition: Remove all remaining edges attached to this dying vertex
                    let dangling_edges = h.edges_containing(real_id);
                    for eid in dangling_edges {
                        if let Some(e) = h.remove_hyperedge(eid) {
                            undo.removed_edges.insert(eid, e);
                        }
                    }
                    
                    // Backup causal data for rollback
                    let mut affected: HashSet<u64> = h.causal_past(real_id).collect();
                    affected.extend(h.causal_future(real_id));
                    affected.insert(real_id);

                    for u_id in affected {
                        if let Some(fb) = h.causal_future_bitset(u_id) {
                            undo.old_causal_future.insert(u_id, fb.clone());
                        }
                        if let Some(pb) = h.causal_past_bitset(u_id) {
                            undo.old_causal_past.insert(u_id, pb.clone());
                        }
                        if let Some(v) = h.vertices.get(&u_id) {
                            undo.old_parents.insert(u_id, v.parents.clone());
                            undo.old_children.insert(u_id, v.children.clone());
                        }
                    }
                    
                    // Kill it permanently
                    if let Some(v) = h.remove_vertex(real_id) {
                        undo.removed_vertices.insert(real_id, v);
                    }
                }
            }
        }

        // 3. SPAWN NEW VERTICES (RHS items not in LHS)
        // If the RHS references an abstract ID that isn't mapped yet, spawn it.
        for rhs_edge in &self.rhs_edges {
            for &abstract_id in rhs_edge {
                if !match_state.mapping.contains_key(&abstract_id) {
                    let new_v = Vertex::new(); // State vector defaults to empty
                    let new_id = new_v.id;
                    h.vertices.insert(new_id, new_v.clone());
                    undo.added_vertices.push(new_id);
                    
                    // Map it so edges can use it instantly!
                    match_state.mapping.insert(abstract_id, new_id);
                }
            }
        }

        // 4. CREATE RHS EDGES
        for rhs_edge in &self.rhs_edges {
            let real_vertices: Vec<u64> = rhs_edge.iter().map(|a_id| *match_state.mapping.get(a_id).unwrap()).collect();
            
            let new_edge = h.add_hyperedge(real_vertices.clone());
            undo.added_edges.push(new_edge.id);
            
            // Causal linking (naive linear causal chain for newly formed edges to simulate flow)
            for i in 0..real_vertices.len().saturating_sub(1) {
                h.add_causal_relation(real_vertices[i], real_vertices[i+1]);
                undo.added_causal.push((real_vertices[i], real_vertices[i+1]));
            }
        }

        undo
    }
}

/// Tracks the state of the backtracking search to ensure perfect mathematical isomorphism
#[derive(Clone, Debug)]
pub struct MatchState {
    /// Maps Abstract Rule IDs -> Real Hypergraph IDs (e.g., 0 -> 1045)
    pub mapping: HashMap<usize, u64>,
    
    /// Tracks which real vertices are already part of the match to ensure Injectivity
    pub used_real_vertices: HashSet<u64>,
}

impl MatchState {
    pub fn new() -> Self {
        Self {
            mapping: HashMap::new(),
            used_real_vertices: HashSet::new(),
        }
    }
}
