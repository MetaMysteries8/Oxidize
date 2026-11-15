"""Run in separate original/candidate processes; stdout is one machine-readable evidence record."""
import contextlib
import importlib
import io
import json
import math
import statistics
import sys
import time

module_name, name, cases_text, performance_text = sys.argv[1:]
cases, policy = json.loads(cases_text), json.loads(performance_text)

def encode(value):
    """Preserve types, dict insertion order, and tuple/list distinctions."""
    kind = type(value)
    if kind in (type(None), bool, int, float, str):
        if kind is float and not math.isfinite(value):
            raise TypeError('non-finite floats need an explicit comparator (unsupported)')
        return [kind.__name__, value]
    if kind in (list, tuple):
        return [kind.__name__, [encode(item) for item in value]]
    if kind is dict:
        return ['dict', [[encode(key), encode(item)] for key, item in value.items()]]
    raise TypeError(f'unsupported differential result type: {kind.__name__}')

import_out, import_err = io.StringIO(), io.StringIO()
with contextlib.redirect_stdout(import_out), contextlib.redirect_stderr(import_err):
    module = importlib.import_module(module_name)
function = getattr(module, name)
observations, successful = [], []
for case in cases:
    original_case = json.loads(json.dumps(case))
    out, err = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
        try:
            result = function(*case['args'], **case.get('kwargs', {}))
        except Exception as error:
            observed = {'error': [type(error).__module__, type(error).__qualname__, str(error)]}
        else:
            observed = {'return': encode(result)}
            successful.append(original_case)
    observed.update(stdout=out.getvalue(), stderr=err.getvalue())
    observations.append(observed)
if not successful:
    raise RuntimeError('at least one successful workload is required for benchmarking')

def batch():
    # JSON cases are decoded anew each call to avoid mutation leaking between repetitions.
    for _ in range(policy['calls_per_sample']):
        for case in successful:
            fresh = json.loads(json.dumps(case))
            function(*fresh['args'], **fresh.get('kwargs', {}))

samples = []
with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
    for _ in range(policy['warmup']):
        batch()
    for _ in range(policy['iterations']):
        start = time.perf_counter_ns()
        batch()
        samples.append(time.perf_counter_ns() - start)
median = statistics.median(samples)
print(json.dumps({'observations': observations, 'import_stdout': import_out.getvalue(),
                  'import_stderr': import_err.getvalue(), 'benchmark': {
                      'samples_ns': samples, 'median_ns': median,
                      'mad_ns': statistics.median(abs(value - median) for value in samples),
                      'successful_cases': len(successful),
                      'calls_per_sample': policy['calls_per_sample'] * len(successful)
                  }}, allow_nan=False))
