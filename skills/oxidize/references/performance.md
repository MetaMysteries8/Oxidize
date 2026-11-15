# Measure the integrated workflow

Choose representative workloads before implementation. Record hardware/runtime/toolchain, warm-up policy, sample count, median, and variability. Include bridge overhead and data conversion. Avoid comparing a Python wrapper against a disconnected Rust microbenchmark.

The CLI measures warmed batches through the actual Python interface with nanosecond monotonic timing and reports raw samples, median, and median absolute deviation. Input reconstruction is included; both sides use the same reconstruction policy. It measures component workloads, not total application speed. Sequential original/candidate timing can be biased by machine load and thermal effects. Repeat independently before drawing production conclusions.

Reject failures of correctness regardless of speed. A lower minimum_speedup may deliberately allow a measured regression, but document that policy. CPU, peak memory, allocations, statistical confidence tests, interleaved A/B runs, whole-application latency, and bridge-only overhead are future work.
