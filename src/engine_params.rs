use std::env;

// ============================================================
// Engine Hyperparameters (externally controlled, API-safe)
// ============================================================

#[derive(Debug, Clone)]
pub struct EngineParams {
    pub topology_mutation_rate: f64,
    pub causal_inertia_scale: f64,
    pub hyperedge_boost: f64,
    pub state_decay_rate: f64,
    pub network_coupling: f64,
    pub memory_coupling: f64,
    pub noise_bias: f64,
    pub random_injection_rate: f64,
    pub causal_horizon: usize,
    pub initial_bitset_capacity: usize,
    pub topology_freeze_threshold: f64,
    pub enforce_strict_topology: bool,
    pub export_observables: bool,
}

impl EngineParams {
    pub fn new() -> Self {
        Self {
            topology_mutation_rate: env::var("CHRE_MUTATION_RATE").unwrap_or_else(|_| "0.15".to_string()).parse().unwrap_or(0.15),
            causal_inertia_scale: env::var("CHRE_INERTIA_SCALE").unwrap_or_else(|_| "1.0".to_string()).parse().unwrap_or(1.0),
            hyperedge_boost: env::var("CHRE_HYPEREDGE_BOOST").unwrap_or_else(|_| "1.02".to_string()).parse().unwrap_or(1.02),
            state_decay_rate: env::var("CHRE_DECAY_RATE").unwrap_or_else(|_| "0.975".to_string()).parse().unwrap_or(0.975),
            network_coupling: env::var("CHRE_NETWORK_COUPLING").unwrap_or_else(|_| "2.2".to_string()).parse().unwrap_or(2.2),
            memory_coupling: env::var("CHRE_MEMORY_COUPLING").unwrap_or_else(|_| "0.3".to_string()).parse().unwrap_or(0.3),
            noise_bias: 0.0,
            random_injection_rate: env::var("CHRE_RANDOM_INJECTION").unwrap_or_else(|_| "0.0".to_string()).parse().unwrap_or(0.0),
            causal_horizon: env::var("CHRE_CAUSAL_HORIZON").unwrap_or_else(|_| "6".to_string()).parse().unwrap_or(6),
            initial_bitset_capacity: env::var("CHRE_BITSET_CAPACITY").unwrap_or_else(|_| "1024".to_string()).parse().unwrap_or(1024),
            topology_freeze_threshold: 0.9,
            enforce_strict_topology: env::var("CHRE_STRICT_TOPOLOGY").unwrap_or_else(|_| "1".to_string()) != "0",
            export_observables: env::var("CHRE_EXPORT_OBSERVABLES").unwrap_or_else(|_| "0".to_string()) == "1",
        }
    }

    pub fn apply_overrides(&mut self, config: &crate::rewrite_engine::EngineConfig) {
        if let Some(v) = config.topology_mutation_rate { self.topology_mutation_rate = v; }
        if let Some(v) = config.causal_inertia_scale { self.causal_inertia_scale = v; }
        if let Some(v) = config.hyperedge_boost { self.hyperedge_boost = v; }
        if let Some(v) = config.state_decay_rate { self.state_decay_rate = v; }
        if let Some(v) = config.network_coupling { self.network_coupling = v; }
        if let Some(v) = config.memory_coupling { self.memory_coupling = v; }
        if let Some(v) = config.random_injection_rate { self.random_injection_rate = v; }
        if let Some(v) = config.causal_horizon { self.causal_horizon = v; }
        if let Some(v) = config.initial_bitset_capacity { self.initial_bitset_capacity = v; }
        if config.disable_patches { self.enforce_strict_topology = false; }
    }
}
