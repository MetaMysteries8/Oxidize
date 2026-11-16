# A checkpoint written by Oxidize v0.1.0

`state.json` is the unmodified checkpoint the `v0.1.0` release wrote (schema 1, no event
history). It is used to prove that newer versions upgrade a v0.1.0 session without losing
anything.

How it was made, on 2026-11-16 on Linux: build the `v0.1.0` tag, copy `fixtures/python-cli`
from that tag to a directory outside `/tmp`, run `oxidize plan .`, then
`oxidize migrate --component compute.py:count_primes --response migration-response.json .`
in Bubblewrap without `--trust-code`.

The attempt is **rejected**. v0.1.0's sandbox could not see `cargo` ("bwrap: execvp cargo"),
one of the blockers fixed later. The file is still a real v0.1.0 checkpoint: it has the config,
the baseline tests, schema-1 baseline evidence (no mutation capture), the saved generated
code and the error. It contains no host paths.

The upgrade test copies this file over a fresh copy of `fixtures/python-cli`, which has the
same files the session was made from.
