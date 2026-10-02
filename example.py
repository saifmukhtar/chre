from chre_api import GraphUniverse, EngineConfig, Rule
import json

# 1. User configures the engine perfectly using a Dataclass
config = EngineConfig(
    verbose=True,
    print_interval=10000,   # Print every 10k steps
    bitset_capacity=1024    # Pre-allocate a reasonable amount (1024 bits = 128 bytes)
)

# 2. Initialize the Universe
universe = GraphUniverse(config)

# 3. Define elegant Rules
creation_rule = Rule(
    lhs=[], 
    kept=[], 
    rhs=[["A", "B"]]
)

expansion_rule = Rule(
    lhs=[["A", "B"]], 
    kept=["A", "B"], 
    rhs=[["A", "B"], ["B", "C"], ["C", "A"]]
)

universe.add_rule(creation_rule)
universe.add_rule(expansion_rule)

# 4. Evolve and Observe!
print("Starting 100,000 step simulation...")
universe.evolve(100_000)

print("\n--- Universe Summary ---")
summary = universe.get_summary()
print(json.dumps(summary, indent=4))
