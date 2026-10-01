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
}

#[pymodule]
fn chre(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Engine>()?;
    Ok(())
}
