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
}

#[pymodule]
fn chre(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Engine>()?;
    Ok(())
}
