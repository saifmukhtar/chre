from dataclasses import dataclass, field
from typing import List, Dict, Optional, Literal
import json
import xml.etree.ElementTree as ET
from xml.dom import minidom
import chre  # The compiled Rust engine (chre.so)

@dataclass
class EngineConfig:
    """Configuration parameters for the CHRE Hypergraph Engine.

    Attributes:
        verbose: If True, the engine logs progress at every print_interval steps.
        print_interval: Number of steps between progress log lines.
        bitset_capacity: Initial allocation size for causal FixedBitSets. Increase
            for large simulations to reduce allocator pressure.
        causal_horizon: Maximum BFS depth when propagating causal relations.
            Higher values increase causal accuracy but raise per-step cost on
            dense graphs.
        semantics: Graph rewriting semantics. Either "DPO" or "SPO".
            "DPO" (Double-Pushout, default): A rewrite is aborted if deleting a
                vertex would leave edges not covered by the rule's LHS.
            "SPO" (Single-Pushout): Dangling edges are deleted automatically when
                their endpoint vertex is removed.
    """
    verbose: bool = True
    print_interval: int = 10000
    bitset_capacity: int = 1024
    causal_horizon: int = 6
    semantics: Literal["DPO", "SPO"] = "DPO"


@dataclass
class Rule:
    """A graph grammar rule defining a structural rewrite.

    Attributes:
        lhs: Left-Hand Side. A list of hyperedges (each a list of vertex names)
            describing the pattern to search for.
        kept: Vertex names from the LHS that are preserved across the rewrite.
            Vertices in the LHS but not in kept are deleted.
        rhs: Right-Hand Side. A list of hyperedges describing the replacement
            structure. Vertex names that appear in the RHS but not the LHS will
            result in new vertices being created.
        weight: Relative probability weight for rule selection. Default is 1.0.
    """
    lhs: List[List[str]]
    kept: List[str]
    rhs: List[List[str]]
    weight: float = 1.0


class GraphUniverse:
    """Python interface for the CHRE Rust engine."""

    def __init__(self, config: Optional[EngineConfig] = None):
        self.config = config or EngineConfig()
        self._engine = chre.Engine()

        self._engine.set_verbose(self.config.verbose)
        self._engine.set_print_interval(self.config.print_interval)
        self._engine.set_bitset_capacity(self.config.bitset_capacity)
        self._engine.set_causal_horizon(self.config.causal_horizon)
        self._engine.set_semantics(self.config.semantics)

    def add_rule(self, rule: Rule):
        """Registers a grammar rule into the engine.

        Raises:
            ValueError: If rule.weight is not a finite positive number.
        """
        if not isinstance(rule.weight, (int, float)) or rule.weight <= 0 or rule.weight != rule.weight:
            raise ValueError(
                f"Rule weight must be a finite positive number, got {rule.weight!r}. "
                f"A weight of 0 means the rule would never fire."
            )
        self._engine.add_rule(rule.lhs, rule.kept, rule.rhs, float(rule.weight))

    def evolve(self, steps: int):
        """Advances the simulation by the given number of steps."""
        self._engine.run(steps)

    def step(self) -> bool:
        """Executes exactly one step.

        Returns:
            bool: True if a rule matched and the graph was modified, False otherwise.
        """
        return self._engine.step()

    def rollback(self) -> bool:
        """Reverts the graph state to exactly before the last successful step.

        Note: CHRE currently only stores one level of undo history. Calling rollback()
        multiple times in a row will only undo the single most recent step.

        Returns:
            bool: True if a rollback was performed, False if no history was available.
        """
        return self._engine.rollback()

    def set_semantics(self, semantics: Literal["DPO", "SPO"]):
        """Switches rewriting semantics at runtime.

        Args:
            semantics: "DPO" for strict Double-Pushout (aborts on dangling edges),
                or "SPO" for Single-Pushout (auto-deletes dangling edges).
        """
        if semantics not in ("DPO", "SPO"):
            raise ValueError(f"semantics must be 'DPO' or 'SPO', got '{semantics}'")
        self.config.semantics = semantics
        self._engine.set_semantics(semantics)

    def get_semantics(self) -> Literal["DPO", "SPO"]:
        """Returns the currently active rewriting semantics as reported by the engine.

        This queries the Rust engine directly, so it reflects the true runtime state
        regardless of how semantics was set (via config or set_semantics()).
        """
        return self._engine.get_semantics()

    def get_summary(self) -> Dict[str, int]:
        """Returns a summary of the current graph state."""
        return {
            "total_vertices": self._engine.node_count(),
            "total_edges": self._engine.edge_count(),
            "structural_pairs": self._engine.get_structural_pair_count()
        }

    def get_raw_topology(self) -> Dict:
        """Returns the raw vertex and edge data."""
        return {
            "vertices": self._engine.get_vertices(),
            "edges": self._engine.get_edges()
        }

    def set_observable(self, v_id: int, key: str, value: float):
        """Attaches arbitrary key-value physical state to a vertex.

        Args:
            v_id: The vertex ID.
            key: The string name of the observable (e.g. 'mass').
            value: The float value.

        Raises:
            ValueError: If the vertex does not exist.
        """
        self._engine.set_observable(v_id, key, float(value))

    def get_observable(self, v_id: int, key: str) -> Optional[float]:
        """Reads a physical observable from a vertex.

        Returns:
            The float value if present, or None if the key is not set.

        Raises:
            ValueError: If the vertex does not exist.
        """
        return self._engine.get_observable(v_id, key)

    def get_all_observables(self) -> Dict[int, Dict[str, float]]:
        """Returns the entire physical state of the universe as a dictionary mapping v_id -> observables."""
        return self._engine.get_all_observables()

    def save_json(self, filepath: str):
        """Exports the graph to a JSON file containing vertices, edges, and observables.
        
        Format:
        {
            "vertices": [{"id": 0, "observables": {"mass": 1.0}}, ...],
            "edges": [[0, 1], [1, 2, 3], ...]
        }
        """
        topo = self.get_raw_topology()
        obs = self.get_all_observables()
        
        out = {
            "vertices": [{"id": v, "observables": obs.get(v, {})} for v in topo["vertices"]],
            "edges": topo["edges"]
        }
        
        with open(filepath, 'w') as f:
            json.dump(out, f, indent=2)

    def save_graphml(self, filepath: str):
        """Exports the graph to GraphML format, which can be opened in Gephi or NetworkX.
        
        Because GraphML has poor native hyperedge support in most visualizers (like Gephi),
        this exports a *bipartite* representation:
        - Vertices are nodes with type='vertex'.
        - Hyperedges are nodes with type='hyperedge'.
        - Binary edges connect the hyperedge-nodes to their vertex-nodes.
        
        Vertex observables are automatically exported as node attributes.
        """
        root = ET.Element("graphml", {
            "xmlns": "http://graphml.graphdrawing.org/xmlns",
            "xmlns:xsi": "http://www.w3.org/2001/XMLSchema-instance",
            "xsi:schemaLocation": "http://graphml.graphdrawing.org/xmlns http://graphml.graphdrawing.org/xmlns/1.0/graphml.xsd"
        })
        
        # Define the 'type' attribute
        key_type = ET.SubElement(root, "key", {"id": "d_type", "for": "node", "attr.name": "type", "attr.type": "string"})
        
        topo = self.get_raw_topology()
        obs = self.get_all_observables()
        
        # Discover all unique observable keys
        all_obs_keys = set()
        for v_obs in obs.values():
            all_obs_keys.update(v_obs.keys())
            
        # Define GraphML keys for each observable
        for k in all_obs_keys:
            ET.SubElement(root, "key", {"id": f"d_{k}", "for": "node", "attr.name": k, "attr.type": "double"})
            
        graph = ET.SubElement(root, "graph", {"id": "G", "edgedefault": "undirected"})
        
        # Add Vertices
        for v in topo["vertices"]:
            n = ET.SubElement(graph, "node", {"id": f"v{v}"})
            ET.SubElement(n, "data", {"key": "d_type"}).text = "vertex"
            v_obs = obs.get(v, {})
            for k, val in v_obs.items():
                ET.SubElement(n, "data", {"key": f"d_{k}"}).text = str(val)
                
        # Add Hyperedges as nodes, and link them to vertices
        edge_id = 0
        for edge_vertices in topo["edges"]:
            e_name = f"e{edge_id}"
            en = ET.SubElement(graph, "node", {"id": e_name})
            ET.SubElement(en, "data", {"key": "d_type"}).text = "hyperedge"
            
            for v in edge_vertices:
                ET.SubElement(graph, "edge", {"source": e_name, "target": f"v{v}"})
                
            edge_id += 1
            
        # Pretty print and save
        xml_str = minidom.parseString(ET.tostring(root, 'utf-8')).toprettyxml(indent="  ")
        # Remove the extra XML declaration minidom adds if we want it perfect, but it's fine.
        with open(filepath, 'w') as f:
            f.write(xml_str)

    def reset(self):
        """Clears all vertices and edges from the graph."""
        self._engine.clear_graph()
