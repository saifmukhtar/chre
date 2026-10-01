# CHRE: Causal Hypergraph Rewrite Engine

**CHRE** (formerly known as `hcsn-rust`) is a high-performance, general-purpose topological rewrite engine. 

Originally built as a physics simulator for modeling relativistic emergence and causal set theory, the core engine has been fully decoupled from theoretical physics. It now serves as a universal, high-speed backend for Artificial Intelligence, complex systems, and network topology research.

## The Architecture
Simulating large-scale hypergraphs usually crashes computers due to combinatorial explosion. CHRE solves this by handling all heavy topological graph rewriting, parallel processing, and causal memory mapping (using heavily optimized bitsets to track causal futures and pasts) natively in **Rust**.

## The Python API (The "Black Box")
CHRE is designed to act as a "Black Box" engine for Python developers. You do not need to know Rust to use it. 

Through `PyO3`, CHRE compiles directly into a native Python module. This allows researchers to:
1. Simply import the engine (`import chre`).
2. Define custom hypergraph rules, parameters, and AI configurations dynamically in Python.
3. Pass control to the engine, which executes hundreds of thousands of simulation steps at native Rust speeds before returning the data to Python.

## Use Cases
- **Causal AI:** Generate and track massively complex Directed Acyclic Graphs (DAGs) for causality research.
- **Dynamic Neural Architecture (Neuroevolution):** Use hypergraph rewriting rules to organically "grow" and restructure neural networks during training.
- **Higher-Order Information Networks:** Simulate how data propagates through dynamic P2P swarms and social topologies without relying on global state.
