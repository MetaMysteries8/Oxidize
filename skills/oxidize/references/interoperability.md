# Choose and verify a real bridge

Python: PyO3; Node: napi-rs; C: stable C ABI; C++: cxx/FFI; Java: JNI; browser: WebAssembly; Rust: normal modules/crates. Only Python/PyO3 is implemented in the current standalone CLI.

Specify ownership, lifetimes, resource cleanup, errors, integer ranges, text encoding, thread affinity, async behavior, and copy costs at each boundary. Python arbitrary precision integers and bool-as-int behavior require deliberate handling. Preserve public signatures and exception contracts; adapter extraction errors are not automatically compatible with Python errors.

For the initial Python adapter, export a complete `#[pymodule]` module plus the requested `#[pyfunction]` through PyO3 0.30.1. The engine builds a cdylib and replaces the original function definition with an import at the same source location. It preserves other functions and the entrypoint. Native artifact loading is currently Linux/macOS only. Packages, decorated/async/generator/default/variadic signatures and general Python object graphs are outside the tested scope.
