"""Time one whole-application command (Milestone 2 §1). stdout is one JSON record.

argv: the command as JSON, warm-up runs, timed runs. The command runs from the current
directory with no stdin and its output discarded. Each timed run records wall time and, where
Python's resource module exists, CPU time (user + system) of that run's process tree. Peak
RSS is the largest single process seen across the warm-up and timed runs (all the same
command), not a sum.
"""
import json
import subprocess
import sys
import time

argv, warmup, runs = json.loads(sys.argv[1]), int(sys.argv[2]), int(sys.argv[3])
try:
    import resource
except ImportError:  # Windows
    resource = None


def children_cpu_ns():
    usage = resource.getrusage(resource.RUSAGE_CHILDREN)
    return (usage.ru_utime + usage.ru_stime) * 1e9


def once():
    before = children_cpu_ns() if resource else None
    start = time.perf_counter_ns()
    done = subprocess.run(argv, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                          stderr=subprocess.PIPE)
    wall = time.perf_counter_ns() - start
    if done.returncode != 0:
        sys.stderr.write(done.stderr.decode('utf-8', 'replace')[-2000:])
        sys.stderr.write(f'\nworkload command exited {done.returncode}\n')
        sys.exit(3)
    cpu = children_cpu_ns() - before if resource else None
    return wall, cpu


for _ in range(warmup):
    once()
wall_ns, cpu_ns = [], []
for _ in range(runs):
    wall, cpu = once()
    wall_ns.append(wall)
    cpu_ns.append(cpu)

unsupported = []
if resource is None:
    cpu_ns = None
    peak_rss_kib = None
    unsupported.append("CPU time and peak RSS: Python's resource module is unavailable on "
                       + sys.platform)
else:
    peak = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
    # macOS reports bytes, Linux and the BSDs KiB.
    peak_rss_kib = peak // 1024 if sys.platform == 'darwin' else peak
print(json.dumps({'wall_ns': wall_ns, 'cpu_ns': cpu_ns, 'peak_rss_kib': peak_rss_kib,
                  'unsupported': unsupported}, allow_nan=False))
