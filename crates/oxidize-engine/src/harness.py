"""Run in separate original/candidate processes; stdout is one machine-readable evidence record."""
import contextlib
import hashlib
import importlib
import io
import json
import math
import platform
import statistics
import stat
import sys
import time
from pathlib import Path

mode, module_name, name, cases_text, performance_text, file_paths_text = sys.argv[1:]
cases, policy = json.loads(cases_text), json.loads(performance_text)
file_paths = json.loads(file_paths_text)

def capture_file_effects():
    """Only explicit regular paths, bounded bytes, no directory trees or symlinks."""
    captured = {}
    for relative in file_paths:
        path = Path.cwd()
        parts = Path(relative).parts
        for index, part in enumerate(parts):
            path = path / part
            try:
                info = path.lstat()
            except FileNotFoundError:
                captured[relative] = None
                break
            if stat.S_ISLNK(info.st_mode):
                raise RuntimeError('file-effect paths must not contain symlinks')
            if index < len(parts) - 1:
                if not stat.S_ISDIR(info.st_mode):
                    raise RuntimeError('file-effect parent must be a directory')
                continue
            if not stat.S_ISREG(info.st_mode):
                raise RuntimeError('file-effect path must be a regular file')
            if info.st_size > 1048576:
                raise RuntimeError('file-effect file exceeds 1 MiB')
            with path.open('rb') as stream:
                data = stream.read(1048577)
            if len(data) > 1048576:
                raise RuntimeError('file-effect file exceeds 1 MiB')
            captured[relative] = {'size_bytes': len(data),
                                  'sha256': hashlib.sha256(data).hexdigest()}
    return captured

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
        before_files = capture_file_effects()
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
        if file_paths:
            after_files = capture_file_effects()
            observed['file_effects'] = {
                path: {'before': before_files[path], 'after': after_files[path]}
                for path in file_paths
            }
        observations.append(observed)
    payload = {'observation_schema_version': 2, 'observations': observations,
               'import_stdout': import_out.getvalue(), 'import_stderr': import_err.getvalue(),
               'file_effect_paths': file_paths}
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
               'calls_per_sample': policy['calls_per_sample'] * len(cases),
               # Where the timing really ran: the interpreter inside the sandbox, not the host's.
               'interpreter': {'implementation': sys.implementation.name,
                               'version': platform.python_version(),
                               'executable': sys.executable}}
else:
    raise ValueError('unknown harness mode')
print(json.dumps(payload, allow_nan=False))
