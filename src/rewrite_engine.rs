use rand::rngs::SmallRng;
use rand::seq::SliceRandom;
use rand::Rng;
use rand::SeedableRng;

use crate::hypergraph::Hypergraph;
use crate::rules::UndoRecord;

pub struct RewriteEngine {
    pub h: Hypergraph,
    pub grammar_rules: Vec<crate::grammar::RewriteRule>,
    pub rng: SmallRng,
    pub time: usize,
    pub verbose: bool,
    pub print_interval: usize,
    pub attempted_rewrites: usize,
}

impl RewriteEngine {
    pub fn new(h: Hypergraph, _dummy: f64, seed: Option<u64>) -> Self {
        let rng = if let Some(s) = seed {
            SmallRng::seed_from_u64(s)
        } else {
            SmallRng::from_entropy()
        };

        Self {
            h,
            grammar_rules: Vec::new(),
            rng,
            time: 0,
            verbose: false,
            print_interval: 10000,
            attempted_rewrites: 0,
        }
    }

    pub fn step(&mut self) -> bool {
        self.time += 1;

        if self.print_interval > 0 && self.time.is_multiple_of(self.print_interval) && self.verbose {
            println!("Step {}...", self.time);
        }

        if let Some(_undo) = self.propose_rewrite() {
            // Undo record could be used here for rollbacks if desired
            return true;
        }
        false
    }

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
        if let Some(rule) = self.grammar_rules.choose(rng) {
            if let Some(match_state) = rule.find_match(&self.h, anchor_v) {
                return Some(rule.apply_match(&mut self.h, match_state));
            }
        }

        None
    }
}
