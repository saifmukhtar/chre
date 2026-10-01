use fixedbitset::FixedBitSet;
use rand::rngs::SmallRng;
use rand::seq::SliceRandom;
use rand::Rng;
use rand::SeedableRng;
use std::collections::{HashMap, HashSet};

use crate::hypergraph::Hypergraph;
use crate::observables::{
    component_radius, compute_coherence_raw, detect_candidate_knot_neighborhoods,
    InteractionEvent, TopologicalKnot,
};
use crate::engine_params::EngineParams;
use crate::rules::UndoRecord;

#[derive(serde::Serialize, Clone)]
pub struct DefectLogEntry {
    pub time: usize,
}

#[derive(serde::Serialize, Clone)]
pub struct XiCurrentLogEntry {
    pub time: usize,
    pub delta_xi: HashMap<u64, f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum ConservationMode {
    Baseline,
    Pairwise,
    StabilityScaled,
    FluxCompensated,
    MassCoupled,
    TimeSymmetry,
    Hybrid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmergenceMode {
    Control,  // Regime A: No stability bonus, hard threshold
    Assisted, // Regime B: Honest inheritance + Sigmoid threshold
    Forced,   // Regime C: Constant bonus + hard threshold
}

pub struct RewriteEngine {
    pub h: Hypergraph,
    pub grammar_rules: Vec<crate::grammar::RewriteRule>,
    pub p_create: f64,
    pub p_fusion: f64,
    pub mode: EmergenceMode,
    pub pure_mode: bool,

    // Physics params
    pub gamma_time: f64,
    pub gamma_ext: f64,
    pub epsilon_label_violation: f64,
    pub params: EngineParams,

    // ξ field
    pub xi: HashMap<u64, f64>,
    pub prev_xi: HashMap<u64, f64>,
    pub xi_threshold: f64,
    pub xi_decay: f64,
    pub xi_coupling: f64,

    // cluster + geometry memory
    pub topo_distance_memory: HashMap<(String, usize, usize), f64>,
    pub xi_distance_memory: HashMap<(String, usize, usize), f64>,
    pub distance_memory_decay: f64,
    pub geometry_stride: usize,

    // knot tracking (Hypothesis 3)
    pub active_knots: HashMap<u64, TopologicalKnot>,
    pub next_knot_id: u64,
    pub dead_knots: Vec<TopologicalKnot>,
    pub interaction_events: Vec<InteractionEvent>,

    // logs
    pub rewrite_history: Vec<UndoRecord>, // Simplified for now
    pub xi_current_log: Vec<XiCurrentLogEntry>,
    pub defect_log: Vec<DefectLogEntry>,

    // rewrite bookkeeping
    pub last_rewrite: Option<UndoRecord>,
    pub forced_time: Option<usize>,
    pub time: usize,
    pub verbose: bool,
    pub print_interval: usize,

    // probes
    pending_bridge: Option<(u64, u64)>,
    pending_bridge_time: Option<usize>,

    pub attempted_rewrites: usize,
    pub suppressed_rewrites: usize,

    // Per-vertex stability memory: accumulates for vertices in coherent structures
    pub stability: HashMap<u64, f64>,

    pub coupling_intensity: HashMap<u64, f64>,
    pub active_interactions: HashMap<(u64, u64), InteractionEvent>,

    // Conservation Mode
    pub conservation_mode: ConservationMode,
    pub p_rev: f64,                            // For Hypothesis E
    pub momentum_reservoir: HashMap<u64, f64>, // For Hypothesis C

    // Observability
    pub thread_id: Option<usize>,
    pub max_steps: usize,
    pub rng: SmallRng,

    // Performance Buffers
    pub bfs_queue: std::collections::VecDeque<u64>,
    pub track_interval: usize,
}

#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub p_create: f64,
    pub p_fusion: Option<f64>,
    pub topology_mutation_rate: Option<f64>,
    pub causal_inertia_scale: Option<f64>,
    pub hyperedge_boost: Option<f64>,
    pub state_decay_rate: Option<f64>,
    pub network_coupling: Option<f64>,
    pub memory_coupling: Option<f64>,
    pub random_injection_rate: Option<f64>,
    pub causal_horizon: Option<usize>,
    pub initial_bitset_capacity: Option<usize>,
    pub disable_patches: bool,
    pub seed: Option<u64>,
    pub track_interval: Option<usize>,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            p_create: 0.64,
            p_fusion: None,
            topology_mutation_rate: None,
            causal_inertia_scale: None,
            hyperedge_boost: None,
            state_decay_rate: None,
            network_coupling: None,
            memory_coupling: None,
            random_injection_rate: None,
            causal_horizon: None,
            initial_bitset_capacity: None,
            disable_patches: false,
            seed: None,
            track_interval: None,
        }
    }
}

impl RewriteEngine {

    pub fn update_topological_knots(&mut self) {
        Self::process_knot_update_static(
            &self.h,
            &mut self.active_knots,
            &mut self.dead_knots,
            &mut self.next_knot_id,
            &self.stability,
            self.time,
            1.2,
            0.3,
        );
        self.perform_kinematics_and_interactions();
    }
    pub fn new(h: Hypergraph, p_create: f64, seed: Option<u64>) -> Self {
        Self::with_config(h, EngineConfig {
            p_create,
            seed,
            ..Default::default()
        })
    }

    pub fn with_config(h: Hypergraph, config: EngineConfig) -> Self {
        println!("DEBUG: RewriteEngine::new start");
        // Initialize deterministic SmallRng from seed or random entropy
        let rng = match config.seed {
            Some(s) => SmallRng::seed_from_u64(s),
            None => SmallRng::from_entropy(),
        };

        let mut engine_params = EngineParams::new();
        engine_params.apply_overrides(&config);

        let mut h = h;
        h.causal_horizon = engine_params.causal_horizon;
        h.initial_bitset_capacity = engine_params.initial_bitset_capacity;

        Self {
            h,
            grammar_rules: Vec::new(),
            p_create: config.p_create,
            p_fusion: config.p_fusion.unwrap_or(0.10), // Reverted default to stable 10%
            gamma_time: 0.1,
            gamma_ext: 0.05,
            epsilon_label_violation: 0.08,
            params: engine_params,
            mode: EmergenceMode::Assisted, // Default to v5.1 Honest Assisted

            xi: HashMap::new(),
            prev_xi: HashMap::new(),
            xi_threshold: 1e-6,
            xi_decay: 0.70,
            xi_coupling: 0.6,

            topo_distance_memory: HashMap::new(),
            xi_distance_memory: HashMap::new(),
            distance_memory_decay: 0.9,
            geometry_stride: 5,

            active_knots: HashMap::new(),
            next_knot_id: 1,
            dead_knots: Vec::new(),
            interaction_events: Vec::new(),

            rewrite_history: Vec::new(),
            xi_current_log: Vec::new(),
            defect_log: Vec::new(),

            last_rewrite: None,
            forced_time: None,
            time: 0,
            verbose: true,
            print_interval: 50,

            pending_bridge: None,
            pending_bridge_time: None,

            attempted_rewrites: 0,
            suppressed_rewrites: 0,
            stability: HashMap::new(),
            active_interactions: HashMap::new(),
            coupling_intensity: HashMap::new(),
            pure_mode: false,

            conservation_mode: ConservationMode::Baseline,
            p_rev: 0.05,
            momentum_reservoir: HashMap::new(),
            thread_id: None,
            max_steps: 0,
            rng,
            bfs_queue: std::collections::VecDeque::new(),
            track_interval: config.track_interval.unwrap_or(10),
        }
    }

    // --------------------------------------------------
    // Main step
    // --------------------------------------------------
    pub fn step(&mut self) -> bool {
        self.time += 1;

        if self.time % 1000 == 0 {
            let tid = self.thread_id.unwrap_or(0);
            println!("Thread {}: Step {}...", tid, self.time);
        }

        self.prev_xi = self.xi.clone();

        let undo_opt = self.propose_rewrite();
        if let Some(ref undo) = undo_opt {
            self.last_rewrite = Some(undo.clone());
            
            // SCRUBBING: Remove deleted vertices from auxiliary maps
            for vid in undo.removed_vertices.keys() {
                self.stability.remove(vid);
                self.xi.remove(vid);
                self.prev_xi.remove(vid);
                self.coupling_intensity.remove(vid);
                self.momentum_reservoir.remove(vid);
            }

            self.rewrite_history.push(undo.clone());
            if self.rewrite_history.len() > 2 {
                self.rewrite_history.drain(0..self.rewrite_history.len() - 2);
            }
        }

        if self.track_interval > 0 && self.time % self.track_interval == 0 {
            self.update_topological_knots();
        }
        self.update_stability();
        self.record_xi_current();

        if self.xi_current_log.len() > 100 {
            self.xi_current_log.drain(0..self.xi_current_log.len() - 100);
        }

        undo_opt.is_some()
    }
    pub fn process_knot_update_static(
        h: &Hypergraph,
        active_knots: &mut HashMap<u64, TopologicalKnot>,
        dead_knots: &mut Vec<TopologicalKnot>,
        next_knot_id: &mut u64,
        _stability: &HashMap<u64, f64>,
        current_time: usize,
        min_coherence: f64,
        overlap_threshold: f64,
    ) {
        let candidates = detect_candidate_knot_neighborhoods(h, min_coherence);
        let mut next_active_knots = HashMap::new();

        // Step 1: Update existing knots
        let mut matched_candidates = HashSet::new();
        for (_, knot) in active_knots.iter() {
            let mut best_idx = None;
            let mut best_overlap = 0.0;

            for (i, cand) in candidates.iter().enumerate() {
                let intersection = knot.vertices.intersection(cand).count() as f64;
                let min_s = knot.vertices.len().min(cand.len()) as f64;
                let overlap = if min_s > 0.0 {
                    intersection / min_s
                } else {
                    0.0
                };

                if overlap > overlap_threshold && overlap > best_overlap {
                    best_overlap = overlap;
                    best_idx = Some(i);
                }
            }

            if let Some(idx) = best_idx {
                matched_candidates.insert(idx);
                let cand = &candidates[idx];
                let (ie, be) = compute_coherence_raw(cand, h);
                let coherence = if be > 0 {
                    ie as f64 / be as f64
                } else if ie > 0 {
                    10.0
                } else {
                    0.0
                };

                let mut updated_knot = knot.clone();
                updated_knot.vertices = cand.clone();
                updated_knot.age += 10;
                updated_knot.radius = component_radius(cand, h);
                updated_knot.coherence = coherence;
                if cand.len() > updated_knot.max_size {
                    updated_knot.max_size = cand.len();
                }
                if cand.len() < updated_knot.min_size {
                    updated_knot.min_size = cand.len();
                }

                let max_id = h.max_vertex_id() as f64;
                let mean_pos =
                    cand.iter().map(|&v| v as f64).sum::<f64>() / (cand.len() as f64 * max_id.max(1.0));
                let prev_pos = knot
                    .position_history
                    .last()
                    .map(|(_, p, _)| *p)
                    .unwrap_or(mean_pos);
                updated_knot.diagnostic_v_abs = (mean_pos - prev_pos).abs() / 10.0;

                updated_knot
                    .position_history
                    .push((current_time, mean_pos, coherence));
                if updated_knot.position_history.len() > 100 {
                    updated_knot
                        .position_history
                        .drain(..updated_knot.position_history.len() - 100);
                }

                next_active_knots.insert(updated_knot.id, updated_knot);
            } else {
                if knot.age >= 50 {
                    dead_knots.push(knot.clone());
                    if dead_knots.len() > 200 {
                        dead_knots.drain(0..dead_knots.len() - 200);
                    }
                }
            }
        }

        // Step 2: Spawn new knots from unmatched candidates
        for (idx, cand) in candidates.iter().enumerate() {
            if matched_candidates.contains(&idx) {
                continue;
            }

            let (ie, be) = compute_coherence_raw(cand, h);
            let coherence = if be > 0 {
                ie as f64 / be as f64
            } else if ie > 0 {
                10.0
            } else {
                0.0
            };

            // Only spawn if it meets structural criteria
            if coherence > min_coherence {
                let max_id = h.max_vertex_id() as f64;
                let mean_pos =
                    cand.iter().map(|&v| v as f64).sum::<f64>() / (cand.len() as f64 * max_id.max(1.0));
                let new_knot = TopologicalKnot {
                    id: *next_knot_id,
                    vertices: cand.clone(),
                    age: 10,
                    radius: component_radius(cand, h),
                    max_size: cand.len(),
                    min_size: cand.len(),
                    coherence,
                    diagnostic_v_abs: 0.0,
                    mass: cand.len() as f64 * coherence.powi(2),
                    momentum: 0.0,
                    energy: 0.0,
                    prev_mass: cand.len() as f64 * coherence.powi(2),
                    prev_momentum: 0.0,
                    position_history: vec![(current_time, mean_pos, coherence)],
                };
                next_active_knots.insert(new_knot.id, new_knot);
                *next_knot_id += 1;
            }
        }

        *active_knots = next_active_knots;
    }

    fn perform_kinematics_and_interactions(&mut self) {
        // Step 3: Formal Kinematics
        self.coupling_intensity.clear();
        let active_ids: Vec<_> = self.active_knots.keys().copied().collect();
        for &id in &active_ids {
            let knot = self.active_knots.get_mut(&id).unwrap();

            // Store previous state for conservation modes
            knot.prev_mass = knot.mass;
            knot.prev_momentum = knot.momentum;

            knot.mass = knot.vertices.len() as f64 * knot.coherence.powi(2);
            let hist = &knot.position_history;
            if hist.len() > 1 {
                // Use CONSECUTIVE frames (last two) for instantaneous velocity.
                // Using first→last spans different normalization baselines (max_id grows
                // monotonically), so the positional delta explodes even after normalizing.
                // Consecutive frames are always within ~10 simulation steps of each other,
                // so max_id barely changes and the delta is small and physically meaningful.
                let (t1, p1, _) = hist[hist.len() - 2];
                let (t2, p2, _) = hist[hist.len() - 1];
                let dt = (t2 - t1) as f64;
                if dt > 0.0 {
                    let mut dv = (p2 - p1) / dt;

                    // Hypothesis B: Exponential Inertia (Inertial Cooling v5.0)
                    if self.params.enforce_strict_topology
                        && (self.conservation_mode == ConservationMode::StabilityScaled
                            || self.conservation_mode == ConservationMode::Hybrid)
                    {
                        let stab = knot
                            .vertices
                            .iter()
                            .map(|&v| self.stability.get(&v).unwrap_or(&0.0))
                            .sum::<f64>()
                            / knot.vertices.len() as f64;
                        // v_new = v_old * exp(-S / 30.0) + v_min
                        dv = dv * (-stab / 30.0).exp() + 0.05;
                    }

                    // Hard clamp: velocity must stay physically bounded.
                    // Normalized position ∈ (0,1], so |Δpos/Δt| should never exceed ~1.
                    // Clamp to ±10.0 as a generous safety ceiling.
                    dv = dv.clamp(-10.0, 10.0);

                    knot.diagnostic_v_abs = dv;
                }
            }
            knot.momentum = knot.mass * knot.diagnostic_v_abs;

            // Energy Definition: E = 0.5 * M * v^2 + lambda * stability
            let stab_sum = knot
                .vertices
                .iter()
                .map(|&v| self.stability.get(&v).unwrap_or(&0.0))
                .sum::<f64>();
            let mean_stability = if !knot.vertices.is_empty() {
                stab_sum / knot.vertices.len() as f64
            } else {
                0.0
            };
            knot.energy = 0.5 * knot.mass * knot.diagnostic_v_abs.powi(2) + 0.02 * mean_stability;
        }

        // --- Hypothesis D: Mass Coupling (Soft Newtonian) ---
        if self.params.enforce_strict_topology
            && self.conservation_mode == ConservationMode::MassCoupled
        {
            for knot in self.active_knots.values_mut() {
                let p_before = knot.prev_momentum;
                let p_after = knot.momentum;
                let correction = (p_before - p_after) * 0.3; // 30% soft constraint
                knot.diagnostic_v_abs += correction / (knot.mass + 1e-6);
                knot.momentum = knot.mass * knot.diagnostic_v_abs;
            }
        }

        // --- Hypothesis C: Local Flux Compensation (Residual Field) ---
        if self.params.enforce_strict_topology
            && (self.conservation_mode == ConservationMode::FluxCompensated
                || self.conservation_mode == ConservationMode::Hybrid)
        {
            for knot in self.active_knots.values_mut() {
                let p_before = knot.prev_momentum;
                let p_after = knot.momentum;
                let delta_p = p_before - p_after;

                // Leak excess to neighborhood reservoir
                if delta_p.abs() > 1e-6 {
                    let per_v = delta_p / (knot.vertices.len() as f64).max(1.0);
                    for &v in &knot.vertices {
                        *self.momentum_reservoir.entry(v).or_insert(0.0) += per_v;
                    }
                }

                // Diffusive re-absorption (alpha = 0.15)
                for &v in &knot.vertices {
                    if let Some(res) = self.momentum_reservoir.get_mut(&v) {
                        let siphon = *res * 0.15;
                        knot.diagnostic_v_abs += siphon / (knot.mass + 1e-6);
                        *res -= siphon;
                    }
                }
            }

            // Reservoir Decay (0.999) - Near-lossless emergence
            for res in self.momentum_reservoir.values_mut() {
                *res *= 0.999;
            }

            // --- Causal Redistribution (v5.0 Phase Transition Support) ---
            // Distribute momentum from vertices that are NO LONGER in the graph
            // into their 1-hop causal neighborhood.
            let mut orphans = Vec::new();
            for (&v, _) in &self.momentum_reservoir {
                if !self.h.vertices.contains_key(&v) {
                    orphans.push(v);
                }
            }

            for v in orphans {
                self.momentum_reservoir.remove(&v);
            }
        }
        // Pre-compute halos for all knots using properly-sized transient bitsets
        let mut active_ids: Vec<u64> = self.active_knots.keys().copied().collect();
        active_ids.sort();

        let max_id = self.h.max_vertex_id() as usize + 1;
        let mut halos = Vec::with_capacity(active_ids.len());
        let mut knot_halos = HashMap::new();

        for (i, &id) in active_ids.iter().enumerate() {
            if let Some(knot) = self.active_knots.get(&id) {
                let mut halo = FixedBitSet::with_capacity(max_id);
                Self::compute_halo_inplace_static(&knot.vertices, &self.h, &mut halo);
                halos.push(halo);
                knot_halos.insert(id, i);
            }
        }

        let mut current_overlaps = HashSet::new();
        for i in 0..active_ids.len() {
            for j in i + 1..active_ids.len() {
                let id_a = active_ids[i];
                let id_b = active_ids[j];
                let halo_a = &halos[i];
                let halo_b = &halos[j];

                // Relational chi based on expanded Interaction Horizons (Halo)
                // O(V/64) intersection count using block-wise comparison (no allocation)
                let intersection_count = halo_a.as_slice().iter()
                    .zip(halo_b.as_slice().iter())
                    .map(|(a, b)| (a & b).count_ones())
                    .sum::<u32>() as usize;

                if intersection_count > 0 {
                    let knot_a = &self.active_knots[&id_a];
                    let knot_b = &self.active_knots[&id_b];
                    
                    let pair = if id_a < id_b { (id_a, id_b) } else { (id_b, id_a) };
                    
                    let chi_rel = intersection_count as f64 
                        / (halo_a.ones().count().min(halo_b.ones().count()) as f64).max(1.0);

                    // Noise filter for active engagement
                    if chi_rel > 0.015 {
                        current_overlaps.insert(pair);

                        if !self.active_interactions.contains_key(&pair) {
                            let m_a = knot_a.mass;
                            let m_b = knot_b.mass;
                            let res = (2.0 * knot_a.coherence * knot_b.coherence)
                                / (knot_a.coherence.powi(2) + knot_b.coherence.powi(2)).max(1e-6);

                            let stab_a = knot_a
                                .vertices
                                .iter()
                                .map(|&v| self.stability.get(&v).unwrap_or(&0.0))
                                .sum::<f64>()
                                / knot_a.vertices.len() as f64;
                            let stab_b = knot_b
                                .vertices
                                .iter()
                                .map(|&v| self.stability.get(&v).unwrap_or(&0.0))
                                .sum::<f64>()
                                / knot_b.vertices.len() as f64;

                            let (int_a, bnd_a) = compute_coherence_raw(&knot_a.vertices, &self.h);
                            let ratio_a = int_a as f64 / (int_a + bnd_a).max(1) as f64;
                            let (int_b, bnd_b) = compute_coherence_raw(&knot_b.vertices, &self.h);
                            let ratio_b = int_b as f64 / (int_b + bnd_b).max(1) as f64;

                            self.active_interactions.insert(
                                pair,
                                InteractionEvent {
                                    start_time: self.time,
                                    end_time: None,
                                    duration: 0,
                                    knot_a: id_a,
                                    knot_b: id_b,
                                    overlap_size: intersection_count,
                                    overlap_depth: chi_rel,
                                    resonance: res,
                                    initial_chi: chi_rel,
                                    prev_chi: chi_rel,
                                    v_rel_smoothed: 0.0,
                                    pre_a: (
                                        m_a,
                                        0.0,
                                        0.0,
                                        knot_a.diagnostic_v_abs,
                                        knot_a.coherence,
                                        stab_a,
                                        knot_a.radius,
                                        knot_a.vertices.len(),
                                        ratio_a,
                                        knot_a.energy,
                                        knot_a.age,
                                    ),
                                    pre_b: (
                                        m_b,
                                        0.0,
                                        0.0,
                                        knot_b.diagnostic_v_abs,
                                        knot_b.coherence,
                                        stab_b,
                                        knot_b.radius,
                                        knot_b.vertices.len(),
                                        ratio_b,
                                        knot_b.energy,
                                        knot_b.age,
                                    ),
                                    post_a: None,
                                    post_b: None,
                                    steps_below_threshold: 0,
                                },
                            );
                        } else {
                            // Update relational kinematics with EMA smoothing to filter rewrite jitter
                            let event = self.active_interactions.get_mut(&pair).unwrap();
                            
                            let raw_v_rel = (chi_rel - event.prev_chi) / 10.0;
                            // Alpha = 0.2 provides low-pass filter against stochastic rewrite noise
                            event.v_rel_smoothed = (0.2 * raw_v_rel) + (0.8 * event.v_rel_smoothed);
                            
                            if chi_rel > event.overlap_depth {
                                event.overlap_depth = chi_rel;
                            }
                            event.prev_chi = chi_rel;
                            event.steps_below_threshold = 0;
                        }
                    }

                    if chi_rel > 0.05 {
                        let intensity = (chi_rel / 0.4).min(1.0);
                        // Apply relief to the entire HALO union (the interaction field)
                        for v_idx in halo_a.ones() {
                            let v = v_idx as u64;
                            let entry = self.coupling_intensity.entry(v).or_insert(0.0);
                            if intensity > *entry { *entry = intensity; }
                        }
                        for v_idx in halo_b.ones() {
                            let v = v_idx as u64;
                            let entry = self.coupling_intensity.entry(v).or_insert(0.0);
                            if intensity > *entry { *entry = intensity; }
                        }
                    }
                }
            }
        }

        // --- Hypothesis A: Pairwise Symmetry (Symmetric Correction) ---
        if self.params.enforce_strict_topology
            && (self.conservation_mode == ConservationMode::Pairwise
                || self.conservation_mode == ConservationMode::Hybrid)
        {
            let mut corrections = Vec::new();
            for (pair, event) in self.active_interactions.iter() {
                if current_overlaps.contains(pair) {
                    if let (Some(ka), Some(kb)) = (
                        self.active_knots.get(&event.knot_a),
                        self.active_knots.get(&event.knot_b),
                    ) {
                        let p_a_before = ka.prev_momentum;
                        let p_b_before = kb.prev_momentum;
                        let p_a_after = ka.momentum;
                        let p_b_after = kb.momentum;

                        let delta_total = (p_a_after + p_b_after) - (p_a_before + p_b_before);

                        // Hybrid A: Asymptotic Symmetry Ramp (v6.0 Phase Transition)
                        let s_avg = (event.pre_a.5 + event.pre_b.5) / 2.0;

                        // k ramps non-linearly with gamma. Physics sharpens as S -> 20.0
                        let k = (s_avg / 20.0).powf(self.params.network_coupling).min(1.0);

                        let correction = -k * 0.5 * delta_total;
                        // Guard: only apply correction if finite (safety net for edge cases)
                        if correction.is_finite() {
                            corrections.push((event.knot_a, event.knot_b, correction));
                        }
                    }
                }
            }

            for (id_a, id_b, corr) in corrections {
                if let Some(ka) = self.active_knots.get_mut(&id_a) {
                    ka.diagnostic_v_abs += corr / (ka.mass + 1e-6);
                    ka.momentum = ka.mass * ka.diagnostic_v_abs;
                }
                if let Some(kb) = self.active_knots.get_mut(&id_b) {
                    kb.diagnostic_v_abs += corr / (kb.mass + 1e-6);
                    kb.momentum = kb.mass * kb.diagnostic_v_abs;
                }
            }
        }

        // Finalize interactions that have ended (stability window)
        let mut finished = Vec::new();
        for (pair, event) in self.active_interactions.iter_mut() {
            if !current_overlaps.contains(pair) {
                event.steps_below_threshold += 1;
                // Since this check runs every step, threshold 10 keeps transient overlaps filtered.
                if event.steps_below_threshold >= 10 {
                    finished.push(*pair);
                }
            }
        }

        for pair in finished {
            let mut event = self.active_interactions.remove(&pair).unwrap();
            event.end_time = Some(self.time);
            event.duration = self.time.saturating_sub(event.start_time);

            // Capture post state
            if let Some(ka) = self.active_knots.get(&event.knot_a) {
                let stab = ka
                    .vertices
                    .iter()
                    .map(|&v| self.stability.get(&v).unwrap_or(&0.0))
                    .sum::<f64>()
                    / ka.vertices.len() as f64;
                let (int, bnd) = compute_coherence_raw(&ka.vertices, &self.h);
                let ratio = int as f64 / (int + bnd).max(1) as f64;
                event.post_a = Some((
                    ka.mass,
                    event.v_rel_smoothed,
                    ka.mass * event.v_rel_smoothed,
                    ka.diagnostic_v_abs,
                    ka.coherence,
                    stab,
                    ka.radius,
                    ka.vertices.len(),
                    ratio,
                    ka.energy,
                    ka.age,
                ));
            }
            if let Some(kb) = self.active_knots.get(&event.knot_b) {
                let stab = kb
                    .vertices
                    .iter()
                    .map(|&v| self.stability.get(&v).unwrap_or(&0.0))
                    .sum::<f64>()
                    / kb.vertices.len() as f64;
                let (int, bnd) = compute_coherence_raw(&kb.vertices, &self.h);
                let ratio = int as f64 / (int + bnd).max(1) as f64;
                event.post_b = Some((
                    kb.mass,
                    event.v_rel_smoothed,
                    kb.mass * event.v_rel_smoothed,
                    kb.diagnostic_v_abs,
                    kb.coherence,
                    stab,
                    kb.radius,
                    kb.vertices.len(),
                    ratio,
                    kb.energy,
                    kb.age,
                ));
            }

            self.interaction_events.push(event);
        }
    }

    /// Computes the 1-hop structural halo for a knot using bitwise OR.
    /// This creates the "Interaction Horizon" needed for Relational Kinematics.
    fn compute_halo_inplace_static(vertices: &HashSet<u64>, h: &Hypergraph, halo: &mut FixedBitSet) {
        halo.clear();
        for &v in vertices {
            let idx = v as usize;
            if idx >= halo.len() {
                halo.grow(idx + 1);
            }
            halo.insert(idx);
            let neighbors = h.structural_neighbors(v);
            for n_id in neighbors {
                let n_idx = n_id as usize;
                if n_idx >= halo.len() {
                    halo.grow(n_idx + 1);
                }
                halo.insert(n_idx);
            }
        }
    }

    /// Update per-vertex stability memory.
    /// Vertices inside active knots accumulate stability.
    /// All stability values decay slowly each cycle.
    fn update_stability(&mut self) {
        // (1) Stability Decay (nu)
        let state_decay_rate = self.params.state_decay_rate;

        // Decay all existing stability
        for val in self.stability.values_mut() {
            *val *= state_decay_rate;
        }

        // Accumulate stability for vertices in active knots
        for knot in self.active_knots.values() {
            // Natural selection baseline: G = 1.0
            let gain = 1.0;
            for &v in &knot.vertices {
                let entry = self.stability.entry(v).or_insert(0.0);
                *entry += gain;
            }
        }

        // Clean up: remove entries for vertices no longer in the graph
        self.stability
            .retain(|v, _| self.h.vertices.contains_key(v));

        // Cap stability to prevent runaway (v5.0 Phase transition expansion)
        for val in self.stability.values_mut() {
            if *val > 50.0 {
                *val = 50.0;
            }
        }
    }

    // --------------------------------------------------
    // Rewrite proposal
    // --------------------------------------------------
    fn propose_rewrite(&mut self) -> Option<UndoRecord> {
        let rng = &mut self.rng;

        self.attempted_rewrites += 1;

        if self.grammar_rules.is_empty() {
            return None;
        }

        // --- Fast Random Anchor Selection (O(1) Amortized) ---
        let anchor_v = if self.h.vertices.is_empty() {
            0 // Dummy anchor for spontaneous creation rules (LHS is empty)
        } else {
            let max_id = self.h.max_vertex_id();
            loop {
                let guess = rng.gen_range(0..max_id);
                if self.h.vertices.contains_key(&guess) {
                    break guess;
                }
            }
        };

        // --- Pure Grammar Execution ---
        use rand::seq::SliceRandom;
        if let Some(rule) = self.grammar_rules.choose(rng) {
            if let Some(match_state) = rule.find_match(&self.h, anchor_v) {
                return Some(rule.apply_match(&mut self.h, match_state));
            }
        }

        None
    }

    // --------------------------------------------------
    // ξ propagation
    // --------------------------------------------------
    fn propagate_xi(&mut self, clusters: &HashMap<u64, usize>) {
        let mut new_xi = self.xi.clone();
        let xi_max = 1e6;

        let protect_clusters = if let Some(ft) = self.forced_time {
            self.time.saturating_sub(ft) < 999999
        } else {
            false
        };

        for (&v, &xi_v) in &self.xi {
            if xi_v < self.xi_threshold {
                continue;
            }

            let cid_v = clusters.get(&v);
            let xi_v_decayed = xi_v * self.xi_decay;

            if let Some(neighbors) = self.h.structural_neighbors_bitset(&v) {
                let deg = (neighbors.ones().count() as f64).max(1.0);
                for u_idx in neighbors.ones() {
                    let u = u_idx as u64;
                    
                    let cid_u = clusters.get(&u);

                    if protect_clusters {
                        if let (Some(cu), Some(cv)) = (cid_u, cid_v) {
                            if cu != cv {
                                continue;
                            }
                        }
                    }

                    *new_xi.entry(u).or_insert(0.0) += 0.15 * xi_v_decayed / deg;
                }
            }

            *new_xi.entry(v).or_insert(0.0) += 0.7 * xi_v_decayed;
        }

        for val in new_xi.values_mut() {
            if *val > xi_max {
                *val = xi_max;
            }
        }

        self.xi = new_xi;
    }

    // --------------------------------------------------
    // ξ clusters
    // --------------------------------------------------
    fn xi_clusters(&self) -> HashMap<u64, usize> {
        let mut clusters = HashMap::new();
        let mut visited = HashSet::new();
        let mut cid = 0;

        let xi_vertices: HashSet<u64> = self
            .xi
            .iter()
            .filter(|(&v, &x)| x > self.xi_threshold && self.h.vertices.contains_key(&v))
            .map(|(v, _)| *v)
            .collect();

        for &v in &xi_vertices {
            if visited.contains(&v) {
                continue;
            }
            let mut stack = vec![v];
            visited.insert(v);
            clusters.insert(v, cid);

            while let Some(u) = stack.pop() {
                if let Some(nbrs) = self.h.structural_neighbors_bitset(&u) {
                    for w_idx in nbrs.ones() {
                        let w = w_idx as u64;
                        if xi_vertices.contains(&w) && !visited.contains(&w) {
                            visited.insert(w);
                            clusters.insert(w, cid);
                            stack.push(w);
                        }
                    }
                }
            }
            cid += 1;
        }

        clusters
    }

    // --------------------------------------------------
    // Topological clusters (PURE TOPOLOGY)
    // --------------------------------------------------
    fn topo_clusters(&self) -> HashMap<u64, usize> {
        let mut clusters = HashMap::new();
        let mut visited = HashSet::new();
        let mut cid = 0;

        for &v in self.h.vertices.keys() {
            if visited.contains(&v) {
                continue;
            }
            let mut stack = vec![v];
            visited.insert(v);
            clusters.insert(v, cid);

            while let Some(u) = stack.pop() {
                if let Some(nbrs) = self.h.structural_neighbors_bitset(&u) {
                    for w_idx in nbrs.ones() {
                        let w = w_idx as u64;
                        if !visited.contains(&w) {
                            visited.insert(w);
                            clusters.insert(w, cid);
                            stack.push(w);
                        }
                    }
                }
            }
            cid += 1;
        }

        clusters
    }

    // --------------------------------------------------
    // Geometry memory (G1)
    // --------------------------------------------------
    fn update_topo_distance_memory(
        &mut self,
        _inter: &HashMap<u64, FixedBitSet>,
        restrict_to: &HashSet<u64>,
    ) {
        let topo = self.topo_clusters();

        let mut topo_groups: HashMap<usize, Vec<u64>> = HashMap::new();
        for &v in restrict_to {
            if let Some(&cid) = topo.get(&v) {
                topo_groups.entry(cid).or_default().push(v);
            }
        }

        let topo_ids: Vec<usize> = topo_groups.keys().cloned().collect();
        if topo_ids.is_empty() {
            return;
        }

        let max_depth = self.geometry_depth();

        // 🔧 Fallback: store intra-component scale if only one topo component
        if topo_ids.len() == 1 {
            let verts = &topo_groups[&topo_ids[0]];
            if verts.len() >= 2 {
                let mid = verts.len() / 2;
                let mut a_verts = verts[..mid].to_vec();
                if a_verts.len() > 25 {
                    a_verts.truncate(25);
                }
                let b_verts: HashSet<u64> = verts[mid..].iter().cloned().collect();

                let mut min_d = usize::MAX;
                for &v in &a_verts {
                    let d = self.graph_distance(v, &b_verts, max_depth);
                    if d < min_d {
                        min_d = d;
                    }
                }

                if min_d < usize::MAX {
                    let key = ("topo".to_string(), topo_ids[0], topo_ids[0]);
                    let d_f = min_d as f64;
                    let prev = *self.topo_distance_memory.get(&key).unwrap_or(&d_f);
                    let new_val = self.distance_memory_decay * prev
                        + (1.0 - self.distance_memory_decay) * d_f;
                    self.topo_distance_memory.insert(key, new_val);
                }
            }
        }

        for i in 0..topo_ids.len() {
            for j in (i + 1)..topo_ids.len() {
                let mut a_verts = topo_groups[&topo_ids[i]].clone();
                if a_verts.len() > 25 {
                    a_verts.truncate(25);
                }
                let b_verts: HashSet<u64> = topo_groups[&topo_ids[j]].iter().cloned().collect();

                let mut min_d = usize::MAX;
                for &v in &a_verts {
                    let d = self.graph_distance(v, &b_verts, max_depth);
                    if d < min_d {
                        min_d = d;
                    }
                }

                if min_d < usize::MAX {
                    let key = ("topo".to_string(), topo_ids[i], topo_ids[j]);
                    let d_f = min_d as f64;
                    let prev = *self.topo_distance_memory.get(&key).unwrap_or(&d_f);
                    let new_val = self.distance_memory_decay * prev
                        + (1.0 - self.distance_memory_decay) * d_f;
                    self.topo_distance_memory.insert(key, new_val);
                }
            }
        }
    }

    // --------------------------------------------------
    // Cluster distance memory (G2)
    // --------------------------------------------------
    fn update_xi_distance_memory(&mut self) {
        let xi_clusters = self.xi_clusters();

        let mut cluster_to_vertices: HashMap<usize, Vec<u64>> = HashMap::new();
        for (&v, &cid) in &xi_clusters {
            cluster_to_vertices.entry(cid).or_default().push(v);
        }

        let cluster_ids: Vec<usize> = cluster_to_vertices.keys().cloned().collect();
        if cluster_ids.is_empty() {
            return;
        }

        let max_depth = self.geometry_depth();

        // 🔧 Fallback: intra-component scale if only one xi cluster
        if cluster_ids.len() == 1 {
            let verts = &cluster_to_vertices[&cluster_ids[0]];
            if verts.len() >= 2 {
                let mid = verts.len() / 2;
                let mut a_verts = verts[..mid].to_vec();
                if a_verts.len() > 25 {
                    a_verts.truncate(25);
                }
                let b_verts: HashSet<u64> = verts[mid..].iter().cloned().collect();

                let mut min_d = usize::MAX;
                for &v in &a_verts {
                    let d = self.graph_distance(v, &b_verts, max_depth);
                    if d < min_d {
                        min_d = d;
                    }
                }

                if min_d < usize::MAX {
                    let key = ("xi".to_string(), cluster_ids[0], cluster_ids[0]);
                    let d_f = min_d as f64;
                    let prev = *self.xi_distance_memory.get(&key).unwrap_or(&d_f);
                    let new_val = self.distance_memory_decay * prev
                        + (1.0 - self.distance_memory_decay) * d_f;
                    self.xi_distance_memory.insert(key, new_val);
                    if self.verbose {
                        println!(
                            "[geom-add] xi_pair (intra-cluster) ({}, {}) d={}",
                            cluster_ids[0], cluster_ids[0], min_d
                        );
                    }
                }
            }
        }

        for i in 0..cluster_ids.len() {
            for j in (i + 1)..cluster_ids.len() {
                let mut a_verts = cluster_to_vertices[&cluster_ids[i]].clone();
                let b_verts: HashSet<u64> = cluster_to_vertices[&cluster_ids[j]]
                    .iter()
                    .cloned()
                    .collect();

                // limit sampling to avoid O(N²) blowup
                if a_verts.len() > 25 {
                    a_verts.truncate(25);
                }

                let mut min_d = usize::MAX;

                for &v in &a_verts {
                    let d = self.graph_distance(v, &b_verts, max_depth);
                    if d < min_d {
                        min_d = d;
                    }
                }

                if min_d < usize::MAX {
                    let key = ("xi".to_string(), cluster_ids[i], cluster_ids[j]);
                    let d_f = min_d as f64;
                    let prev = *self.xi_distance_memory.get(&key).unwrap_or(&d_f);
                    let new_val = self.distance_memory_decay * prev
                        + (1.0 - self.distance_memory_decay) * d_f;
                    self.xi_distance_memory.insert(key, new_val);

                    if self.verbose {
                        println!(
                            "[geom-add] xi_pair ({}, {}) d={}",
                            cluster_ids[i], cluster_ids[j], min_d
                        );
                    }
                }
            }
        }
    }

    fn record_xi_current(&mut self) {
        let touched = self.touched_vertices();
        let mut delta_xi = HashMap::new();
        for v in touched {
            if self.prev_xi.contains_key(&v) {
                let current = self.xi.get(&v).unwrap_or(&0.0);
                let prev = self.prev_xi.get(&v).unwrap_or(&0.0);
                let delta = current - prev;
                if delta.is_finite() {
                    delta_xi.insert(v, delta);
                }
            }
        }

        if !delta_xi.is_empty() {
            self.xi_current_log.push(XiCurrentLogEntry {
                time: self.time,
                delta_xi,
            });
        }
    }

    fn touched_vertices(&self) -> HashSet<u64> {
        let mut touched = HashSet::new();
        if let Some(lr) = &self.last_rewrite {
            for &v in &lr.added_vertices {
                touched.insert(v);
            }
            for &v_id in lr.removed_vertices.keys() {
                touched.insert(v_id);
            }
        }
        touched
    }

    fn geometry_depth(&self) -> usize {
        let n = self.h.vertices.len() as f64;
        16.max((n + 1.0).log2() as usize * 4)
    }

    fn graph_distance(
        &self,
        start_v: u64,
        targets: &HashSet<u64>,
        max_depth: usize,
    ) -> usize {
        if targets.contains(&start_v) {
            return 0;
        }

        let mut visited = HashSet::new();
        visited.insert(start_v);

        let mut frontier = HashSet::new();
        frontier.insert(start_v);

        let mut depth = 0;

        while !frontier.is_empty() && depth < max_depth {
            depth += 1;
            let mut next_frontier = HashSet::new();

            for &v in &frontier {
                for u in self.h.structural_neighbors(v) {
                    if true {
                        
                        if visited.contains(&u) {
                            continue;
                        }
                        if targets.contains(&u) {
                            return depth;
                        }
                        visited.insert(u);
                        next_frontier.insert(u);
                    }
                }
            }
            frontier = next_frontier;
        }
        usize::MAX
    }

    fn undo_changes(&mut self, record: UndoRecord) {
        // --- Stability Inheritance (v5.0) ---
        // Transfer 90% of stability from the anchor/target vertices to the new vertices
        let mut target_stability = 0.0;
        let targets: Vec<u64> = record
            .target
            .iter()
            .map(|&id| self.h.vertices.get(&id).map(|v| v.id).unwrap_or(0))
            .collect();

        if !targets.is_empty() {
            target_stability = targets
                .iter()
                .map(|&v| self.stability.get(&v).copied().unwrap_or(0.0))
                .sum::<f64>()
                / targets.len() as f64;
        }

        // Inheritance flows to newly added vertices (S_new = 0.9 * S_old)
        let inheritance = target_stability * 0.9;
        for &new_v_id in &record.added_vertices {
            let s = self.stability.entry(new_v_id).or_insert(0.0);
            *s = (*s + inheritance).min(50.0);
        }

        // Apply hypergraph changes
        self.h.execute_undo_record(record);
    }


    pub fn force_second_proto_object(
        &mut self,
        _omega_kick: f64, // Not currently used directly in python injection logic
        xi_seed: f64,
        min_distance: usize,
        max_tries: usize,
    ) -> bool {
        let mut xi_support = HashSet::new();
        for (&vid, &x) in &self.xi {
            if x > self.xi_threshold && self.h.vertices.contains_key(&vid) {
                xi_support.insert(vid);
            }
        }

        if xi_support.is_empty() {
            return false;
        }

        let _inter = crate::observables::worldline_interaction_graph(&self.h, 0.0);
        let n = self.h.vertices.len().max(1) as f64;
        let max_depth = 20.max((n.log2() * 4.0) as usize);

        // restrict candidates to the same connected component to avoid `inf` distances
        let mut reachable = xi_support.clone();
        let mut frontier = xi_support.clone();
        let mut depth = 0;

        while !frontier.is_empty() && depth < max_depth {
            depth += 1;
            let mut nxt = HashSet::new();
            for &v in &frontier {
                for u in self.h.structural_neighbors(v) {
                    if true {
                        
                        if !reachable.contains(&u) {
                            reachable.insert(u);
                            nxt.insert(u);
                        }
                    }
                }
            }
            frontier = nxt;
        }

        let mut candidates: Vec<u64> = reachable.difference(&xi_support).cloned().collect();
        if candidates.is_empty() {
            // island is completely isolated, fallback to global graph
            candidates = self.h.vertices.keys().cloned().collect();
        }

        let mut best_vid = None;
        let mut best_d = 0;

        for _ in 0..max_tries {
            let vid = *candidates.choose(&mut self.rng).unwrap();
            if xi_support.contains(&vid) {
                continue;
            }

            let d = self.graph_distance(vid, &xi_support, max_depth);

            if d > best_d || best_vid.is_none() {
                best_d = d;
                best_vid = Some(vid);
            }

            if d >= min_distance && d != usize::MAX {
                self.xi.insert(vid, xi_seed);
                self.forced_time = Some(self.time);
                if self.verbose {
                    println!("### SECOND PROBE at t={} | v={} | d={}", self.time, vid, d);
                }
                return true;
            }
        }

        if let Some(vid) = best_vid {
            self.xi.insert(vid, xi_seed);

            if let Some(&u) = xi_support.iter().next() {
                self.pending_bridge = Some((u, vid));
                self.pending_bridge_time = Some(self.time);
            }

            self.forced_time = Some(self.time);
            if self.verbose {
                println!(
                    "### SECOND PROBE (fallback) at t={} | v={} | d={}",
                    self.time, vid, best_d
                );
            }
            return true;
        }

        false
    }

    pub fn compute_local_suppression_for_knot(
        &self,
        knot: &crate::observables::TopologicalKnot,
    ) -> f64 {
        let alpha_base = 1.5;
        let lambda = 0.5;
        let survival_threshold = 1.0;

        let avg_neighborhood = if knot.vertices.is_empty() {
            0.0
        } else {
            knot.vertices
                .iter()
                .map(|v| self.h.coordination_number(*v))
                .sum::<usize>() as f64
                / knot.vertices.len() as f64
        };

        let coherence_boost = if avg_neighborhood >= 4.0 && knot.coherence > survival_threshold {
            lambda * (knot.coherence - survival_threshold)
        } else {
            0.0
        };

        let mu = self.params.memory_coupling;
        let gamma = self.params.network_coupling;
        let stability_cap = 30.0_f64;
        let mean_stability = if knot.vertices.is_empty() {
            0.0
        } else {
            knot.vertices
                .iter()
                .map(|v| self.stability.get(v).copied().unwrap_or(0.0))
                .sum::<f64>()
                / knot.vertices.len() as f64
        };

        let normalized_stability = (mean_stability / stability_cap).min(1.0);
        let memory_contribution = mu * stability_cap * normalized_stability.powf(gamma);

        let alpha_eff = alpha_base + coherence_boost + memory_contribution;
        let coupling_modifier = 1.0;
        let local_density = 1.0;

        (-(alpha_eff * coupling_modifier) * local_density).exp()
    }

    pub fn export_mechanism_correlation_data(&self) -> serde_json::Value {
        let mut data = Vec::new();

        for knot in self.active_knots.values() {
            if knot.age < 50 {
                continue;
            }
            let mean_stab = if knot.vertices.is_empty() {
                0.0
            } else {
                knot.vertices
                    .iter()
                    .map(|v| self.stability.get(v).copied().unwrap_or(0.0))
                    .sum::<f64>()
                    / knot.vertices.len() as f64
            };

            let suppression = self.compute_local_suppression_for_knot(knot);
            let memory = knot.age as f64;
            let damping = (-mean_stab / 30.0).exp();

            data.push(serde_json::json!({
                "knot_id": knot.id,
                "age": knot.age,
                "stability": mean_stab,
                "coherence": knot.coherence,
                "suppression": suppression,
                "memory": memory,
                "damping": damping,
                "survived": true,
            }));
        }

        for knot in &self.dead_knots {
            if knot.age < 50 {
                continue;
            }
            let mean_stab = if knot.vertices.is_empty() {
                0.0
            } else {
                knot.vertices
                    .iter()
                    .map(|v| self.stability.get(v).copied().unwrap_or(0.0))
                    .sum::<f64>()
                    / knot.vertices.len() as f64
            };

            let suppression = self.compute_local_suppression_for_knot(knot);
            let memory = knot.age as f64;
            let damping = (-mean_stab / 30.0).exp();

            data.push(serde_json::json!({
                "knot_id": knot.id,
                "age": knot.age,
                "stability": mean_stab,
                "coherence": knot.coherence,
                "suppression": suppression,
                "memory": memory,
                "damping": damping,
                "survived": false,
            }));
        }

        serde_json::json!(data)
    }
}
