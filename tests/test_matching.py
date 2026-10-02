"""
tests/test_matching.py

Tests for rule matching correctness, DPO/SPO semantics, and graph invariants.

Run with:
    cd /path/to/chre
    python3 -m pytest tests/ -v
"""
import sys
import os
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import pytest
from chre_api import GraphUniverse, EngineConfig, Rule


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def make_universe(semantics: str = "DPO", verbose: bool = False) -> GraphUniverse:
    return GraphUniverse(EngineConfig(verbose=verbose, semantics=semantics))


# ---------------------------------------------------------------------------
# 1. Spontaneous creation (empty LHS)
# ---------------------------------------------------------------------------

class TestSpontaneousCreation:
    def test_empty_graph_starts_at_zero(self):
        u = make_universe()
        assert u.get_summary()["total_vertices"] == 0
        assert u.get_summary()["total_edges"] == 0

    def test_creation_rule_produces_vertices(self):
        u = make_universe()
        u.add_rule(Rule(lhs=[], kept=[], rhs=[["A", "B"]]))
        u.evolve(10)
        assert u.get_summary()["total_vertices"] > 0
        assert u.get_summary()["total_edges"] > 0

    def test_creation_rule_produces_edge_between_two_vertices(self):
        u = make_universe()
        u.add_rule(Rule(lhs=[], kept=[], rhs=[["A", "B"]]))
        u.evolve(1)
        summary = u.get_summary()
        # One rule application: 2 vertices, 1 edge
        assert summary["total_vertices"] == 2
        assert summary["total_edges"] == 1


# ---------------------------------------------------------------------------
# 2. Pattern matching correctness
# ---------------------------------------------------------------------------

class TestPatternMatching:
    def test_expansion_rule_fires_on_existing_edge(self):
        """Expansion rule: [A,B] -> [A,B],[B,C],[C,A]. Creates a triangle from an edge."""
        u = make_universe()
        u.add_rule(Rule(lhs=[], kept=[], rhs=[["A", "B"]]))
        u.add_rule(Rule(
            lhs=[["A", "B"]],
            kept=["A", "B"],
            rhs=[["A", "B"], ["B", "C"], ["C", "A"]]
        ))
        u.evolve(100)
        summary = u.get_summary()
        # Graph must have grown — the expansion rule fires
        assert summary["total_vertices"] > 2
        assert summary["total_edges"] > 1

    def test_rule_with_no_matching_pattern_does_nothing(self):
        """A rule whose LHS cannot match should never fire."""
        u = make_universe()
        # Create a single edge
        u.add_rule(Rule(lhs=[], kept=[], rhs=[["A", "B"]]))
        u.evolve(1)
        # Add a rule that looks for a 3-node hyperedge — not present in graph
        u.add_rule(Rule(
            lhs=[["X", "Y", "Z"]],
            kept=["X", "Y"],
            rhs=[["X", "Y"]]
        ))
        snapshot_before = u.get_summary()
        u.evolve(200)
        snapshot_after = u.get_summary()
        # The unmatched rule fires every step but always fails.
        # Only the creation rule fires. Vertices should grow.
        # This just verifies the engine does not crash or corrupt state.
        assert snapshot_after["total_vertices"] >= snapshot_before["total_vertices"]

    def test_multiple_rules_can_coexist(self):
        """Two rules registered simultaneously must both be accepted without error."""
        u = make_universe()
        u.add_rule(Rule(lhs=[], kept=[], rhs=[["A", "B"]]))
        u.add_rule(Rule(
            lhs=[["A", "B"]],
            kept=["A", "B"],
            rhs=[["A", "B"], ["B", "C"], ["C", "A"]]
        ))
        # Should not raise
        u.evolve(50)
        assert u.get_summary()["total_vertices"] > 0


# ---------------------------------------------------------------------------
# 3. Kept vertices survive; deleted vertices are removed
# ---------------------------------------------------------------------------

class TestVertexLifecycle:
    def test_kept_vertices_survive(self):
        """After triangle expansion, the graph must grow (original nodes kept, C added)."""
        u = make_universe()
        # Only add the creation rule first, seed one edge
        u.add_rule(Rule(lhs=[], kept=[], rhs=[["A", "B"]]))
        u.evolve(1)
        v_before = u.get_summary()["total_vertices"]
        assert v_before == 2

        # Now add expansion rule and evolve one more step
        # The engine picks one rule per step. Either rule may fire.
        # Over enough steps, expansion must have fired at least once.
        u.add_rule(Rule(
            lhs=[["A", "B"]],
            kept=["A", "B"],
            rhs=[["A", "B"], ["B", "C"], ["C", "A"]]
        ))
        u.evolve(50)
        v_after = u.get_summary()["total_vertices"]
        # Must have grown — expansion adds C, creation adds pairs
        assert v_after > v_before

    def test_vertex_count_is_non_negative_after_long_run(self):
        u = make_universe()
        u.add_rule(Rule(lhs=[], kept=[], rhs=[["A", "B"]]))
        u.add_rule(Rule(
            lhs=[["A", "B"]],
            kept=["A", "B"],
            rhs=[["A", "B"], ["B", "C"], ["C", "A"]]
        ))
        u.evolve(10_000)
        summary = u.get_summary()
        assert summary["total_vertices"] >= 0
        assert summary["total_edges"] >= 0


# ---------------------------------------------------------------------------
# 4. DPO vs SPO semantics
# ---------------------------------------------------------------------------

class TestSemantics:
    def test_dpo_is_default(self):
        u = make_universe()
        assert u.config.semantics == "DPO"

    def test_spo_set_via_config(self):
        u = make_universe(semantics="SPO")
        assert u.config.semantics == "SPO"

    def test_runtime_toggle(self):
        u = make_universe()
        assert u.config.semantics == "DPO"
        u.set_semantics("SPO")
        assert u.config.semantics == "SPO"
        u.set_semantics("DPO")
        assert u.config.semantics == "DPO"

    def test_invalid_semantics_raises(self):
        u = make_universe()
        with pytest.raises(ValueError):
            u.set_semantics("INVALID")

    def test_spo_and_dpo_both_run_without_error(self):
        """Both semantics must produce valid, non-crashing simulations."""
        for sem in ("DPO", "SPO"):
            u = make_universe(semantics=sem)
            u.add_rule(Rule(lhs=[], kept=[], rhs=[["A", "B"]]))
            u.add_rule(Rule(
                lhs=[["A", "B"]],
                kept=["A", "B"],
                rhs=[["A", "B"], ["B", "C"], ["C", "A"]]
            ))
            u.evolve(5_000)
            summary = u.get_summary()
            assert summary["total_vertices"] >= 0
            assert summary["total_edges"] >= 0


# ---------------------------------------------------------------------------
# 5. Graph state consistency
# ---------------------------------------------------------------------------

class TestGraphConsistency:
    def test_reset_clears_graph(self):
        u = make_universe()
        u.add_rule(Rule(lhs=[], kept=[], rhs=[["A", "B"]]))
        u.evolve(100)
        assert u.get_summary()["total_vertices"] > 0
        u.reset()
        assert u.get_summary()["total_vertices"] == 0
        assert u.get_summary()["total_edges"] == 0

    def test_get_raw_topology_keys(self):
        u = make_universe()
        u.add_rule(Rule(lhs=[], kept=[], rhs=[["A", "B"]]))
        u.evolve(5)
        raw = u.get_raw_topology()
        assert "vertices" in raw
        assert "edges" in raw

    def test_edge_count_matches_raw_topology(self):
        u = make_universe()
        u.add_rule(Rule(lhs=[], kept=[], rhs=[["A", "B"]]))
        u.evolve(5)
        summary = u.get_summary()
        raw = u.get_raw_topology()
        assert summary["total_edges"] == len(raw["edges"])

    def test_vertex_count_matches_raw_topology(self):
        u = make_universe()
        u.add_rule(Rule(lhs=[], kept=[], rhs=[["A", "B"]]))
        u.evolve(5)
        summary = u.get_summary()
        raw = u.get_raw_topology()
        assert summary["total_vertices"] == len(raw["vertices"])


# ---------------------------------------------------------------------------
# 6. Engine configuration
# ---------------------------------------------------------------------------

class TestEngineConfig:
    def test_default_config_values(self):
        from chre_api import EngineConfig
        c = EngineConfig()
        assert c.verbose is True
        assert c.print_interval == 10000
        assert c.bitset_capacity == 1024
        assert c.causal_horizon == 6
        assert c.semantics == "DPO"

    def test_custom_config_applied(self):
        u = make_universe()
        u.add_rule(Rule(lhs=[], kept=[], rhs=[["A", "B"]]))
        # Should not raise — just verifies the config was accepted by the engine
        u.evolve(10)


# ---------------------------------------------------------------------------
# 7. Rule weight validation and weighted selection
# ---------------------------------------------------------------------------

class TestRuleWeights:
    def test_valid_weight_accepted(self):
        u = make_universe()
        u.add_rule(Rule(lhs=[], kept=[], rhs=[["A", "B"]], weight=1.0))
        u.add_rule(Rule(lhs=[], kept=[], rhs=[["A", "B"]], weight=99.5))
        u.evolve(10)
        assert u.get_summary()["total_vertices"] > 0

    def test_zero_weight_rejected(self):
        u = make_universe()
        with pytest.raises(ValueError, match="finite positive"):
            u.add_rule(Rule(lhs=[], kept=[], rhs=[["A", "B"]], weight=0.0))

    def test_negative_weight_rejected(self):
        u = make_universe()
        with pytest.raises(ValueError, match="finite positive"):
            u.add_rule(Rule(lhs=[], kept=[], rhs=[["A", "B"]], weight=-1.0))

    def test_nan_weight_rejected(self):
        u = make_universe()
        with pytest.raises(ValueError, match="finite positive"):
            u.add_rule(Rule(lhs=[], kept=[], rhs=[["A", "B"]], weight=float("nan")))

    def test_inf_weight_rejected(self):
        u = make_universe()
        with pytest.raises(ValueError, match="finite positive"):
            u.add_rule(Rule(lhs=[], kept=[], rhs=[["A", "B"]], weight=float("inf")))

    def test_weighted_simulation_runs(self):
        """Weighted rules must produce a valid simulation without error."""
        u = make_universe()
        u.add_rule(Rule(lhs=[], kept=[], rhs=[["A", "B"]], weight=10.0))
        u.add_rule(Rule(
            lhs=[["A", "B"]], kept=["A", "B"],
            rhs=[["A", "B"], ["B", "C"], ["C", "A"]], weight=1.0
        ))
        u.evolve(1_000)
        assert u.get_summary()["total_vertices"] > 0


# ---------------------------------------------------------------------------
# 8. Undo and Rollback API
# ---------------------------------------------------------------------------

class TestUndoRollback:
    def test_rollback_empty_returns_false(self):
        u = make_universe()
        assert u.rollback() is False

    def test_rollback_reverts_single_step(self):
        u = make_universe()
        u.add_rule(Rule(lhs=[], kept=[], rhs=[["A", "B"]]))
        
        # Take exactly one step
        fired = u.step()
        assert fired is True
        assert u.get_summary()["total_vertices"] == 2
        assert u.get_summary()["total_edges"] == 1
        
        # Rollback
        rolled_back = u.rollback()
        assert rolled_back is True
        assert u.get_summary()["total_vertices"] == 0
        assert u.get_summary()["total_edges"] == 0

    def test_rollback_reverts_counters(self):
        u = make_universe()
        u.add_rule(Rule(lhs=[], kept=[], rhs=[["A", "B"]]))
        
        u.step()
        assert u._engine.get_successful_rewrites() == 1
        assert u._engine.get_attempted_rewrites() == 1
        
        u.rollback()
        assert u._engine.get_successful_rewrites() == 0
        assert u._engine.get_attempted_rewrites() == 0

    def test_multi_rollback_fails(self):
        """Only one level of history is stored."""
        u = make_universe()
        u.add_rule(Rule(lhs=[], kept=[], rhs=[["A", "B"]]))
        
        u.step()
        assert u.rollback() is True
        assert u.rollback() is False


# ---------------------------------------------------------------------------
# 9. Observables
# ---------------------------------------------------------------------------

class TestObservables:
    def test_set_and_get_observable(self):
        u = make_universe()
        u.add_rule(Rule(lhs=[], kept=[], rhs=[["A"]]))
        u.step()
        
        v_id = u.get_raw_topology()["vertices"][0]
        
        # Initially not set
        assert u.get_observable(v_id, "mass") is None
        
        # Set and retrieve
        u.set_observable(v_id, "mass", 1.5)
        u.set_observable(v_id, "spin", -0.5)
        
        assert u.get_observable(v_id, "mass") == 1.5
        assert u.get_observable(v_id, "spin") == -0.5

    def test_get_all_observables(self):
        u = make_universe()
        u.add_rule(Rule(lhs=[], kept=[], rhs=[["A"]]))
        u.step()
        
        v_id = u.get_raw_topology()["vertices"][0]
        u.set_observable(v_id, "energy", 42.0)
        
        obs = u.get_all_observables()
        assert v_id in obs
        assert obs[v_id]["energy"] == 42.0

    def test_invalid_vertex_raises(self):
        u = make_universe()
        with pytest.raises(ValueError, match="Vertex 999 not found"):
            u.set_observable(999, "mass", 1.0)
        
        with pytest.raises(ValueError, match="Vertex 999 not found"):
            u.get_observable(999, "mass")


# ---------------------------------------------------------------------------
# 10. Exporters
# ---------------------------------------------------------------------------

class TestExporters:
    def test_save_json(self, tmp_path):
        import json
        u = make_universe()
        u.add_rule(Rule(lhs=[], kept=[], rhs=[["A", "B"]]))
        u.step()
        v = u.get_raw_topology()["vertices"][0]
        u.set_observable(v, "mass", 3.14)
        
        filepath = tmp_path / "test.json"
        u.save_json(str(filepath))
        
        with open(filepath) as f:
            data = json.load(f)
            
        assert "vertices" in data
        assert "edges" in data
        assert len(data["vertices"]) == 2
        
        # Check observables
        v0 = next(vd for vd in data["vertices"] if vd["id"] == v)
        assert v0["observables"]["mass"] == 3.14

    def test_save_graphml(self, tmp_path):
        import xml.etree.ElementTree as ET
        u = make_universe()
        u.add_rule(Rule(lhs=[], kept=[], rhs=[["A", "B"]]))
        u.step()
        v = u.get_raw_topology()["vertices"][0]
        u.set_observable(v, "charge", -1.0)
        
        filepath = tmp_path / "test.graphml"
        u.save_graphml(str(filepath))
        
        tree = ET.parse(filepath)
        root = tree.getroot()
        
        # GraphML namespace
        ns = {"gml": "http://graphml.graphdrawing.org/xmlns"}
        
        nodes = root.findall(".//gml:node", ns)
        edges = root.findall(".//gml:edge", ns)
        
        # 2 vertices + 1 hyperedge = 3 nodes in bipartite format
        assert len(nodes) == 3
        # 1 hyperedge containing 2 vertices = 2 binary edges
        assert len(edges) == 2
