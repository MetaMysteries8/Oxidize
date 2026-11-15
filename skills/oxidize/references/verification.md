# Correctness evidence

Keep existing tests independent from generated implementation code. Test normal inputs, boundaries, errors, wrong types, mutation, and interface behavior. Differentially compare return types and values, exceptions and messages, stdout/stderr, exit codes, serialized data, and externally observable effects as applicable. Use explicit tolerances or normalization only when the contract requires them.

The current CLI compares Python scalar/container results with type tags, dictionary insertion order, exceptions, and captured stdout/stderr on supplied JSON cases. It also runs a configured independent test command before and after integration. It rejects known zero-test output from unittest/pytest. Arbitrary custom runners remain the user's responsibility.

It does not prove general equivalence. It does not compare file mutations, network behavior, concurrency, identity, aliasing, timing behavior, function metadata, or arbitrary Python objects. Add independent evidence for these before expanding the adapter. Generated tests are supplemental evidence, never a substitute for independent expectations.
