"""Run in separate original/candidate processes; stdout is one machine-readable evidence record."""
import contextlib
import importlib
import io
import json
import math
import statistics
import sys
import time

mode, module_name, name, cases_text, performance_text = sys.argv[1:]
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
if mode == 'observe':
    observations = []
    for template in cases:
        # Each observation has fresh JSON inputs; mutations on an error path count too.
        case = json.loads(json.dumps(template))
        args, kwargs = case['args'], case.get('kwargs', {})
        out, err = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            try:
                result = function(*args, **kwargs)
            except Exception as error:
                observed = {'error': [type(error).__module__, type(error).__qualname__, str(error)]}
            else:
                observed = {'return': encode(result)}
        observed.update(arguments_after={'args': encode(args), 'kwargs': encode(kwargs)},
                        stdout=out.getvalue(), stderr=err.getvalue())
        observations.append(observed)
    payload = {'observation_schema_version': 2, 'observations': observations,
               'import_stdout': import_out.getvalue(), 'import_stderr': import_err.getvalue()}
elif mode == 'benchmark':
    # The engine supplies only cases that returned successfully during observation.
    # Timing runs in a different process and fresh workspace from correctness capture.
    if not cases:
        raise RuntimeError('at least one successful workload is required for benchmarking')

    def batch():
        for _ in range(policy['calls_per_sample']):
            for template in cases:
                fresh = json.loads(json.dumps(template))
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
    payload = {'samples_ns': samples, 'median_ns': median,
               'mad_ns': statistics.median(abs(value - median) for value in samples),
               'successful_cases': len(cases),
               'calls_per_sample': policy['calls_per_sample'] * len(cases)}
else:
    raise ValueError('unknown harness mode')
print(json.dumps(payload, allow_nan=False))
