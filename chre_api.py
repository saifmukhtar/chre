from dataclasses import dataclass
from typing import List, Dict, Optional
import chre  # The compiled Rust engine (chre.so)

@dataclass
class EngineConfig:
    """Configuration parameters for the CHRE Hypergraph Engine."""
    verbose: bool = True
    print_interval: int = 10000
    bitset_capacity: int = 1024
    causal_horizon: int = 6

@dataclass
class Rule:
    """A Topological Graph Grammar Rule."""
    lhs: List[List[str]]          # The pattern to match
    kept: List[str]               # The vertices to preserve (Dangling Condition applies to others)
    rhs: List[List[str]]          # The new structure to generate
    weight: float = 1.0           # Probability weight

class GraphUniverse:
    """Pythonic Wrapper for the high-performance Rust CHRE Engine."""
    
    def __init__(self, config: Optional[EngineConfig] = None):
        self.config = config or EngineConfig()
        self._engine = chre.Engine()
        
        # Inject configurations into Rust
        self._engine.set_verbose(self.config.verbose)
        self._engine.set_print_interval(self.config.print_interval)
        self._engine.set_bitset_capacity(self.config.bitset_capacity)
        self._engine.set_causal_horizon(self.config.causal_horizon)

    def add_rule(self, rule: Rule):
        """Registers a physics/grammar rule into the engine."""
        self._engine.add_rule(rule.lhs, rule.kept, rule.rhs, rule.weight)

    def evolve(self, steps: int):
        """Advances the universe by N steps at native Rust speeds."""
        self._engine.run(steps)

    def get_summary(self) -> Dict[str, int]:
        """Returns a quick macroscopic summary of the universe."""
        return {
            "total_vertices": self._engine.node_count(),
            "total_edges": self._engine.edge_count(),
            "structural_pairs": self._engine.get_structural_pair_count()
        }

    def get_raw_topology(self):
        """Returns the raw graph data for NetworkX or saving to disk."""
        return {
            "vertices": self._engine.get_vertices(),
            "edges": self._engine.get_edges()
        }
    
    def reset(self):
        """Wipes the universe clean."""
        self._engine.clear_graph()
