use crate::hypergraph::Hypergraph;
use fixedbitset::FixedBitSet;
use rand::seq::SliceRandom;
use std::collections::{HashMap, HashSet};

pub fn average_coordination(h: &Hypergraph) -> f64 {
    // Compute average coordination number <k>.
    h.average_coordination()
}

pub fn causal_interval_size(h: &Hypergraph, u_id: u64, v_id: u64) -> usize {
    // |I(u, v)| = |J+(u) ∩ J-(v)|
    let past_v: HashSet<u64> = h.causal_past(v_id).collect();
    h.causal_future(u_id).filter(|id| past_v.contains(id)).count()
}

pub fn myrheim_meyer_dimension(h: &Hypergraph, samples: usize, min_interval: usize) -> Option<f64> {
    /*
    Myrheim–Meyer dimension estimator with interval filtering.
    Only considers sufficiently large causal intervals.
    */
    let vertices: Vec<u64> = h.vertices.keys().cloned().collect();
    if vertices.len() < 2 {
        return None;
    }

    let mut sizes = Vec::new();
    let mut rng = rand::thread_rng();

    for _ in 0..samples {
        let sample: Vec<&u64> = vertices.choose_multiple(&mut rng, 2).collect();
        let u_id = *sample[0];
        let v_id = *sample[1];

        if h.is_causally_related(u_id, v_id) {
            let i = causal_interval_size(h, u_id, v_id);
            if i >= min_interval {
                sizes.push(i);
            }
        }
    }

    if sizes.is_empty() {
        return None;
    }

    let avg_i = sizes.iter().sum::<usize>() as f64 / sizes.len() as f64;
    let n = h.vertices.len() as f64;

    if avg_i <= 1.0 {
        return None;
    }

    let dim = 2.0 * n.ln() / avg_i.ln();
    if dim.is_finite() {
        Some(dim)
    } else {
        None
    }
}

pub fn average_large_interval(h: &Hypergraph, samples: usize, min_interval: usize) -> f64 {
    /*
    Measure average size of large causal intervals.
    Returns 0 if none exist.
    */
    let vertices: Vec<u64> = h.vertices.keys().cloned().collect();
    if vertices.len() < 2 {
        return 0.0;
    }

    let mut sizes = Vec::new();
    let mut rng = rand::thread_rng();

    for _ in 0..samples {
        let sample: Vec<&u64> = vertices.choose_multiple(&mut rng, 2).collect();
        let u_id = *sample[0];
        let v_id = *sample[1];

        if h.is_causally_related(u_id, v_id) {
            let i = causal_interval_size(h, u_id, v_id);
            if i >= min_interval {
                sizes.push(i);
            }
        }
    }

    if sizes.is_empty() {
        return 0.0;
    }

    sizes.iter().sum::<usize>() as f64 / sizes.len() as f64
}

pub fn adjacency_overlap(h_before: &Hypergraph, h_after: &Hypergraph) -> f64 {
    // Fraction of hyperedges that persist after a rewrite.
    if h_before.hyperedges.is_empty() {
        return 1.0;
    }

    let mut before = HashSet::new();
    for e in h_before.hyperedges.values() {
        let mut sorted_verts = e.vertices.clone();
        sorted_verts.sort_unstable();
        before.insert(sorted_verts);
    }

    let mut after = HashSet::new();
    for e in h_after.hyperedges.values() {
        let mut sorted_verts = e.vertices.clone();
        sorted_verts.sort_unstable();
        after.insert(sorted_verts);
    }

    let intersection_count = before.intersection(&after).count();
    let max_len = before.len().max(1);

    intersection_count as f64 / max_len as f64
}

pub fn interaction_concentration(interactions: &HashMap<u64, FixedBitSet>) -> f64 {
    /*
    Φ = max degree / total degree
    interactions: dict {node_id: set(neighbors)}
    */
    let degrees: Vec<usize> = interactions.values().map(|v| v.ones().count()).collect();
    if degrees.is_empty() {
        return 0.0;
    }
    let sum_degrees: usize = degrees.iter().sum();
    if sum_degrees == 0 {
        return 0.0;
    }

    let max_degree = *degrees.iter().max().unwrap();
    max_degree as f64 / sum_degrees as f64
}

pub fn worldline_interaction_graph(h: &Hypergraph, fraction: f64) -> HashMap<u64, FixedBitSet> {
    // Build interaction graph among deep worldlines.
    let max_depth = h.max_chain_length() as f64;
    let cutoff = (fraction * max_depth) as usize;

    let wl_ids: HashSet<u64> = h
        .vertices
        .values()
        .filter(|v| v.depth >= cutoff)
        .map(|v| v.id)
        .collect();

    let mut interactions: HashMap<u64, FixedBitSet> = HashMap::new();
    let max_id = h.max_vertex_id();
    let cap = (max_id as usize + 256).max(1024);

    for edge in h.hyperedges.values() {
        let ids: Vec<u64> = edge
            .vertices
            .iter()
            .filter(|id| wl_ids.contains(id))
            .cloned()
            .collect();

        for i in &ids {
            for j in &ids {
                if i != j {
                    let bs = interactions
                        .entry(*i)
                        .or_insert_with(|| FixedBitSet::with_capacity(cap));
                    
                    if (*j as usize) >= bs.len() {
                        bs.grow((*j as usize + 1).max(bs.len() * 2));
                    }
                    bs.insert(*j as usize);
                }
            }
        }
    }

    interactions
}

pub fn count_triangles(interactions: &HashMap<u64, FixedBitSet>) -> usize {
    /*
    Count triangles in an undirected interaction graph.
    interactions: dict {node_id: set(neighbors)}
    */
    let mut triangles = 0;

    for (&u, nbrs_u) in interactions {
        for v_idx in nbrs_u.ones() {
            let v = v_idx as u64;
            if v <= u {
                continue;
            }
            if let Some(nbrs_v) = interactions.get(&v) {
                // common neighbors w form triangles u-v-w
                // Use bitset-native intersection count if possible, but manual check is fine for triangles logic
                for w_idx in nbrs_u.ones() {
                    let w = w_idx as u64;
                    if w > v && nbrs_v.contains(w_idx) {
                        triangles += 1;
                    }
                }
            }
        }
    }
    triangles
}

/*
Ω is a consistent structural observable derived from local clustering statistics.
While not unique, it provides a stable, scale-local diagnostic of structural organization.
*/
pub fn compute_omega(h: &Hypergraph) -> f64 {
    let mut total = 0.0;
    let mut count = 0;

    for &v_id in h.vertices.keys() {
        let neighbors = h.structural_neighbors(v_id);
        let k = neighbors.len();

        if k < 2 {
            continue;
        }

        let mut edges_between_neighbors = 0;
        let mut possible = 0;

        let neigh_vec: Vec<u64> = neighbors.into_iter().collect();

        for i in 0..neigh_vec.len() {
            for j in (i + 1)..neigh_vec.len() {
                possible += 1;
                if h.is_structurally_interacting(neigh_vec[i], neigh_vec[j]) {
                    edges_between_neighbors += 1;
                }
            }
        }

        let local_c = (edges_between_neighbors as f64) / (possible as f64);
        total += local_c;
        count += 1;
    }

    if count == 0 {
        0.0
    } else {
        total / count as f64
    }
}

pub fn compute_omega_graph(adj: &HashMap<u64, FixedBitSet>) -> f64 {
    let mut total = 0.0;
    let mut count = 0;

    for (&_v_id, neighbors) in adj {
        let k = neighbors.ones().count();
        if k < 2 {
            continue;
        }

        let mut edges_between_neighbors = 0;
        let possible = (k * (k - 1)) / 2;

        let neigh_ids: Vec<u64> = neighbors.ones().map(|x| x as u64).collect();
        for i in 0..neigh_ids.len() {
            for j in (i + 1)..neigh_ids.len() {
                let u = neigh_ids[i];
                let v = neigh_ids[j];
                if let Some(u_neighbors) = adj.get(&u) {
                    if u_neighbors.contains(v as usize) {
                        edges_between_neighbors += 1;
                    }
                }
            }
        }

        let local_c = (edges_between_neighbors as f64) / (possible as f64);
        total += local_c;
        count += 1;
    }

    if count == 0 {
        0.0
    } else {
        total / count as f64
    }
}

pub fn state_divergence(h: &Hypergraph) -> usize {
    let mut divergences = 0;
    for edge in h.hyperedges.values() {
        if edge.vertices.is_empty() { continue; }
        let first_v = edge.vertices[0];
        let first_state = h.vertices.get(&first_v).map(|v| v.state.clone()).unwrap_or_default();
        
        for &v_id in &edge.vertices[1..] {
            if let Some(v) = h.vertices.get(&v_id) {
                if v.state != first_state {
                    divergences += 1;
                    break;
                }
            }
        }
    }
    divergences
}

pub fn state_divergence_density(h: &Hypergraph) -> f64 {
    if h.hyperedges.is_empty() {
        return 0.0;
    }
    state_divergence(h) as f64 / h.hyperedges.len() as f64
}

pub fn local_omega(_h: &Hypergraph, inter: &HashMap<u64, FixedBitSet>, v: u64) -> f64 {
    /*
    Local contribution to Ω. Defined as the fraction of interactions
    involving vertex v that participate in closed local motifs.
    */
    let neighbors = match inter.get(&v) {
        Some(set) => set,
        None => return 0.0,
    };

    if neighbors.is_empty() {
        return 0.0;
    }

    let mut closed = 0;
    for u_idx in neighbors.ones() {
        if let Some(n_u) = inter.get(&(u_idx as u64)) {
            if n_u.contains(v as usize) {
                closed += 1;
            }
        }
    }

    closed as f64 / (neighbors.ones().count() as f64).max(1.0)
}

pub fn compute_coherence_raw(
    neighborhood: &HashSet<u64>,
    h: &Hypergraph,
) -> (u32, u32) {
    let mut internal: u32 = 0;
    let mut boundary: u32 = 0;
    for &n in neighborhood {
        let neighbors = h.structural_neighbors(n);
        for nn_id in neighbors {
            if neighborhood.contains(&nn_id) {
                internal += 1;
            } else {
                boundary += 1;
            }
        }
    }
    (internal / 2, boundary)
}

#[derive(Debug, Clone)]
pub struct TopologicalKnot {
    pub id: u64,
    pub vertices: HashSet<u64>,
    pub age: usize,
    pub max_size: usize,
    pub min_size: usize,
    pub radius: f64,
    // Relational tracking metadata
    pub coherence: f64,
    pub diagnostic_v_abs: f64,    // [DIAGNOSTIC PROXY] scalar magnitude (Vertex-ID)
    pub mass: f64,                // size * coherence^eta
    pub momentum: f64,            // [DIAGNOSTIC] m * v_abs
    pub energy: f64,              // [DIAGNOSTIC] 0.5 * m * v_abs^2
    pub prev_mass: f64,
    pub prev_momentum: f64,
    pub position_history: Vec<(usize, f64, f64)>, // (time, centroid_x_approx, coherence)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct InteractionEvent {
    pub start_time: usize,
    pub end_time: Option<usize>,
    pub duration: usize,
    pub knot_a: u64,
    pub knot_b: u64,
    pub overlap_size: usize,
    pub overlap_depth: f64, // chi = max overlap reached
    pub resonance: f64,     // A = (2*coh_a*coh_b)/(coh_a^2 + coh_b^2)
    
    // Relational Kinematics (Machian)
    pub initial_chi: f64,     // chi at t_0
    pub prev_chi: f64,        // chi at t-10
    pub v_rel_smoothed: f64,  // EMA-smoothed d(chi)/dt

    // Kinematic & Structural states
    // [m, v_rel, p_rel, diagnostic_v_abs, coherence, mean_stability, radius, size, boundary_ratio, energy, age]
    pub pre_a: (f64, f64, f64, f64, f64, f64, f64, usize, f64, f64, usize),
    pub pre_b: (f64, f64, f64, f64, f64, f64, f64, usize, f64, f64, usize),
    pub post_a: Option<(f64, f64, f64, f64, f64, f64, f64, usize, f64, f64, usize)>,
    pub post_b: Option<(f64, f64, f64, f64, f64, f64, f64, usize, f64, f64, usize)>,

    // For internal lifecycle tracking
    #[serde(skip)]
    pub steps_below_threshold: usize,
}

pub fn component_radius(comp: &HashSet<u64>, h: &Hypergraph) -> f64 {
    if comp.len() < 2 {
        return 0.0;
    }
    let mut total_dist = 0;
    let mut pairs = 0;

    for &start in comp {
        let mut visited = HashSet::new();
        visited.insert(start);
        let mut queue = vec![(start, 0)];
        let mut head = 0;

        while head < queue.len() {
            let (node, dist) = queue[head];
            head += 1;
            total_dist += dist;
            if dist > 0 {
                pairs += 1;
            }

            let neighbors = h.structural_neighbors(node);
            for nbr in neighbors {
                if comp.contains(&nbr) && !visited.contains(&nbr) {
                    visited.insert(nbr);
                    queue.push((nbr, dist + 1));
                }
            }
        }
    }
    if pairs == 0 {
        0.0
    } else {
        (total_dist as f64) / (pairs as f64)
    }
}

pub fn local_clustering(h: &Hypergraph, v: u64) -> f64 {
    let neighbors = h.structural_neighbors(v);
    let k = neighbors.len();
    if k < 2 {
        return 0.0;
    }

    let mut links = 0;
    let nbr_vec: Vec<u64> = neighbors.into_iter().collect();
    for i in 0..k {
        for j in i + 1..k {
            if h.is_structurally_interacting(nbr_vec[i], nbr_vec[j]) {
                links += 1;
            }
        }
    }

    (2.0 * links as f64) / (k * (k - 1)) as f64
}

pub fn detect_candidate_knot_neighborhoods(
    h: &Hypergraph,
    min_coherence: f64,
) -> Vec<HashSet<u64>> {
    // Step 1: For each vertex, evaluate its 1-hop neighborhood as a candidate region
    let theta_comp = 0.6; // compactness threshold

    let mut seed_regions: Vec<FixedBitSet> = Vec::new();
    let max_v = h.max_vertex_id() as usize + 1;

    for &v in h.vertices.keys() {
        let mut neighborhood = h.structural_neighbors(v);
        neighborhood.insert(v);

        if neighborhood.len() < 3 {
            continue;
        }

        // Count internal vs boundary edges
        let (internal, boundary) = compute_coherence_raw(&neighborhood, h);

        let coherence = if boundary > 0 {
            internal as f64 / boundary as f64
        } else if internal > 0 {
            10.0 // pure clique
        } else {
            0.0
        };

        let total = internal + boundary;
        let compactness = if total > 0 {
            internal as f64 / total as f64
        } else {
            0.0
        };

        // Hard threshold — NO statistics
        if coherence > min_coherence && compactness > theta_comp {
            let mut bs = FixedBitSet::with_capacity(max_v);
            for &member in &neighborhood {
                bs.insert(member as usize);
            }
            seed_regions.push(bs);
        }
    }

    // Step 2: Merge overlapping seed regions via Union-Find and Lookup Map
    if seed_regions.is_empty() {
        return Vec::new();
    }

    let n = seed_regions.len();
    let mut parent: Vec<usize> = (0..n).collect();

    fn find(i: usize, p: &mut Vec<usize>) -> usize {
        if p[i] == i {
            i
        } else {
            let root = find(p[i], p);
            p[i] = root;
            root
        }
    }

    fn union(i: usize, j: usize, p: &mut Vec<usize>) {
        let root_i = find(i, p);
        let root_j = find(j, p);
        if root_i != root_j {
            p[root_i] = root_j;
        }
    }

    // Map vertex -> list of seed region indices it belongs to
    let mut v_to_s: HashMap<u64, Vec<usize>> = HashMap::new();
    for (idx, seed) in seed_regions.iter().enumerate() {
        for v_idx in seed.ones() {
            v_to_s.entry(v_idx as u64).or_default().push(idx);
        }
    }

    // Evaluate merges only for seeds sharing at least one vertex
    for seeds in v_to_s.values() {
        for i in 0..seeds.len() {
            for j in i + 1..seeds.len() {
                let s1 = seeds[i];
                let s2 = seeds[j];
                if find(s1, &mut parent) == find(s2, &mut parent) {
                    continue;
                }

                let mut overlap_bs = seed_regions[s1].clone();
                overlap_bs.intersect_with(&seed_regions[s2]);
                let overlap = overlap_bs.ones().count();
                let min_s = seed_regions[s1].ones().count().min(seed_regions[s2].ones().count());
                if min_s > 0 && (overlap as f64 / min_s as f64) > 0.3 {
                    union(s1, s2, &mut parent);
                }
            }
        }
    }

    // Grouping seeds by Union-Find root
    let mut groups: HashMap<usize, FixedBitSet> = HashMap::new();
    for i in 0..n {
        let root = find(i, &mut parent);
        let group = groups.entry(root).or_insert_with(|| FixedBitSet::with_capacity(max_v));
        group.union_with(&seed_regions[i]);
    }

    groups.into_values().map(|bs| bs.ones().map(|i| i as u64).collect()).collect()
}
