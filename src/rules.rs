use crate::hypergraph::{Hyperedge, Vertex};
use fixedbitset::FixedBitSet;
use std::collections::HashMap;

/// Explict struct to replace Python's dynamic undo dictionary
#[derive(Clone, Debug, Default)]
pub struct UndoRecord {
    pub target: Vec<u64>,
    pub added_vertices: Vec<u64>,
    pub added_edges: Vec<u64>,
    pub added_causal: Vec<(u64, u64)>,
    pub removed_vertices: HashMap<u64, Vertex>,
    pub kept_vertex: Option<u64>,
    pub removed_edges: HashMap<u64, Hyperedge>,
    pub old_causal_future: HashMap<u64, FixedBitSet>,
    pub old_causal_past: HashMap<u64, FixedBitSet>,
    pub old_parents: HashMap<u64, Vec<u64>>,
    pub old_children: HashMap<u64, Vec<u64>>,
}
