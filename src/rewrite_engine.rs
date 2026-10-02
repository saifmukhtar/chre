use rand::distr::{Distribution, weighted::WeightedIndex};
use rand::rngs::StdRng;
use rand::seq::IndexedRandom;
use rand::SeedableRng;

use crate::hypergraph::Hypergraph;
use crate::rules::UndoRecord;

pub struct RewriteEngine {
    pub h: Hypergraph,
    pub grammar_rules: Vec<crate::grammar::RewriteRule>,
    pub rng: StdRng,
    pub time: usize,
    pub verbose: bool,
    pub print_interval: usize,
    pub attempted_rewrites: usize,
    pub successful_rewrites: usize,
    pub strict_dpo: bool,
    pub last_undo: Option<UndoRecord>,
}

impl RewriteEngine {
    pub fn new(h: Hypergraph, _dummy: f64, seed: Option<u64>) -> Self {
        let rng = if let Some(s) = seed {
            StdRng::seed_from_u64(s)
        } else {
            StdRng::from_rng(&mut rand::rng())
        };

        Self {
            h,
            grammar_rules: Vec::new(),
            rng,
            time: 0,
            verbose: false,
            print_interval: 10000,
            attempted_rewrites: 0,
            successful_rewrites: 0,
            strict_dpo: true,
            last_undo: None,
        }
    }

    pub fn step(&mut self) -> bool {
        self.time += 1;

        if self.print_interval > 0 && self.time.is_multiple_of(self.print_interval) && self.verbose
        {
            println!("Step {}...", self.time);
        }

        if let Some(undo) = self.propose_rewrite() {
            self.last_undo = Some(undo);
            return true;
        }
        self.last_undo = None; // clear if no rule fired
        false
    }

    pub fn rollback(&mut self) -> bool {
        if let Some(undo) = self.last_undo.take() {
            self.h.execute_undo_record(undo);
            // Reverse the counters
            self.successful_rewrites = self.successful_rewrites.saturating_sub(1);
            self.attempted_rewrites = self.attempted_rewrites.saturating_sub(1);
            self.time = self.time.saturating_sub(1);
            return true;
        }
        false
    }

    fn propose_rewrite(&mut self) -> Option<UndoRecord> {
        self.attempted_rewrites += 1;

        if self.grammar_rules.is_empty() {
            return None;
        }

        // --- True Random Anchor Selection (O(1)) ---
        let anchor_v = if self.h.active_vertex_ids.is_empty() {
            0 // Dummy anchor for spontaneous creation rules (LHS is empty)
        } else {
            *self.h.active_vertex_ids.choose(&mut self.rng).unwrap_or(&0)
        };

        // --- Weighted Rule Selection ---
        // WeightedIndex is built from the stored weights each call.
        // Rule count is always small (< ~20), so this is negligible overhead.
        // Invariant: all weights are > 0.0 (enforced at add_rule time).
        let rule_index = {
            let weights: Vec<f64> = self.grammar_rules.iter().map(|r| r.weight).collect();
            let dist = WeightedIndex::new(&weights)
                .expect("WeightedIndex build failed — rule weights must all be > 0.0");
            dist.sample(&mut self.rng)
        };

        let rule = &self.grammar_rules[rule_index];
        if let Some(match_state) = rule.find_match(&self.h, anchor_v, self.strict_dpo) {
            self.successful_rewrites += 1;
            return Some(rule.apply_match(&mut self.h, match_state));
        }

        None
    }
}
