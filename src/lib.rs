pub mod hypergraph;
pub mod rewrite_engine;
pub mod rules;
pub mod grammar;

use pyo3::prelude::*;
use crate::rewrite_engine::RewriteEngine;
use crate::hypergraph::Hypergraph;
use crate::grammar::RewriteRule;

#[pyclass]
pub struct Engine {
    inner: RewriteEngine,
}

#[pymethods]
impl Engine {
    #[new]
    pub fn new() -> Self {
        let h = Hypergraph::new();
        // Create engine with 0.0 purely random creation (we are using pure grammar now)
        let inner = RewriteEngine::new(h, 0.0, None);
        Self { inner }
    }

    pub fn add_rule(&mut self, lhs: Vec<Vec<String>>, kept: Vec<String>, rhs: Vec<Vec<String>>, weight: f64) {
        let rule = RewriteRule::new(lhs, kept, rhs, weight);
        self.inner.grammar_rules.push(rule);
    }

    pub fn run(&mut self, steps: usize) {
        for _ in 0..steps {
            self.inner.step();
        }
    }

    pub fn node_count(&self) -> usize {
        self.inner.h.vertices.len()
    }

    pub fn edge_count(&self) -> usize {
        self.inner.h.hyperedges.len()
    }

    /// Extracts all vertex IDs as a Python List
    pub fn get_vertices(&self) -> Vec<u64> {
        self.inner.h.vertices.keys().copied().collect()
    }

    /// Extracts all hyperedges as a Python List of Lists
    pub fn get_edges(&self) -> Vec<Vec<u64>> {
        self.inner.h.hyperedges.values().map(|e| e.vertices.clone()).collect()
    }

    /// Returns a Python Dictionary mapping Vertex ID -> Degree (Number of connected edges)
    pub fn get_degree_map(&self) -> std::collections::HashMap<u64, usize> {
        let mut degrees = std::collections::HashMap::new();
        for &v_id in self.inner.h.vertices.keys() {
            degrees.insert(v_id, self.inner.h.edges_containing(v_id).len());
        }
        degrees
    }

    /// Fast macroscopic metric: total number of pairwise connections
    pub fn get_structural_pair_count(&self) -> usize {
        self.inner.h.structural_pair_count
    }

    /// Clear all graph grammar rules
    pub fn clear_rules(&mut self) {
        self.inner.grammar_rules.clear();
    }

    /// Clear the hypergraph (Reset engine state without re-allocating engine)
    pub fn clear_graph(&mut self) {
        self.inner.h = Hypergraph::new();
        self.inner.attempted_rewrites = 0;
        self.inner.time = 0;
    }

    /// Returns isolated vertices (degree == 0)
    pub fn get_isolated_vertices(&self) -> Vec<u64> {
        self.inner.h.vertices.keys()
            .filter(|&&v_id| self.inner.h.edges_containing(v_id).is_empty())
            .copied()
            .collect()
    }

    /// Calculates the topological graph distance (shortest path) between two vertices
    /// Returns None if there is no path.
    pub fn get_shortest_path_distance(&self, start: u64, target: u64) -> Option<usize> {
        if !self.inner.h.vertices.contains_key(&start) || !self.inner.h.vertices.contains_key(&target) {
            return None;
        }
        if start == target {
            return Some(0);
        }

        let mut visited = std::collections::HashSet::new();
        visited.insert(start);
        
        let mut frontier = vec![start];
        let mut depth = 0;

        while !frontier.is_empty() {
            depth += 1;
            let mut next_frontier = Vec::new();

            for v in frontier {
                for nbr in self.inner.h.structural_neighbors(v) {
                    if nbr == target {
                        return Some(depth);
                    }
                    if visited.insert(nbr) {
                        next_frontier.push(nbr);
                    }
                }
            }
            frontier = next_frontier;
        }
        None
    }

    /// Extracts edge sizes: Edge ID -> Number of contained vertices
    pub fn get_edge_size_map(&self) -> std::collections::HashMap<u64, usize> {
        let mut sizes = std::collections::HashMap::new();
        for (&e_id, edge) in &self.inner.h.hyperedges {
            sizes.insert(e_id, edge.vertices.len());
        }
        sizes
    }
}

#[pymodule]
fn chre(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Engine>()?;
    Ok(())
}
