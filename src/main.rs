use chre::grammar::RewriteRule;
use chre::hypergraph::Hypergraph;
use chre::rewrite_engine::RewriteEngine;

fn main() {
    println!("=== CHRE Pure Graph Grammar Engine ===");

    let h = Hypergraph::new();
    let mut engine = RewriteEngine::new(h, 1.0, None);

    // Spontaneous Creation Rule
    let rule1 = RewriteRule::new(
        vec![],
        vec![],
        vec![vec!["A".to_string(), "B".to_string()]],
        1.0,
    );

    // Triangle Expansion Rule
    let rule2 = RewriteRule::new(
        vec![vec!["A".to_string(), "B".to_string()]],
        vec!["A".to_string(), "B".to_string()],
        vec![
            vec!["A".to_string(), "B".to_string()],
            vec!["B".to_string(), "C".to_string()],
            vec!["C".to_string(), "A".to_string()],
        ],
        1.0,
    );

    engine.grammar_rules.push(rule1);
    engine.grammar_rules.push(rule2);

    println!("Running 100,000 steps...");
    for _ in 0..100_000 {
        engine.step();
    }

    println!("Simulation Complete!");
    println!("Nodes: {}", engine.h.vertices.len());
    println!("Edges: {}", engine.h.hyperedges.len());
}
