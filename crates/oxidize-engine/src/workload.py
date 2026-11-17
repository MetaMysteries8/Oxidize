"""Time one whole-application command (Milestone 2 §1). stdout is one JSON record.

argv: the command as JSON, warm-up runs, timed runs, stderr limit in bytes. The command runs
from the current directory with no stdin and its stdout discarded. Its stderr is drained as it
arrives, keeping only the last 2 KiB for diagnostics. A command whose stderr passes the limit is
stopped, and that is an error, the same output budget the runner enforces on everything else. Each timed run records wall time and, where
Python's resource module exists, CPU time (user + system) of that run's process tree. Peak
RSS is the largest single process seen across the warm-up and timed runs (all the same
command), not a sum.
"""
import json
import subprocess
import sys
import threading
import time

argv, warmup, runs = json.loads(sys.argv[1]), int(sys.argv[2]), int(sys.argv[3])
stderr_limit = int(sys.argv[4])
TAIL = 2048
try:
    import resource
except ImportError:  # Windows
    resource = None


def children_cpu_ns():
    usage = resource.getrusage(resource.RUSAGE_CHILDREN)
    return (usage.ru_utime + usage.ru_stime) * 1e9


def stop(child):
    # Only the direct child: its descendants stay in this process group, which the runner
    # kills as a whole when this wrapper exits (a new session would escape that).
    try:
        child.kill()
    except OSError:
        pass


def once():
    before = children_cpu_ns() if resource else None
    start = time.perf_counter_ns()
    child = subprocess.Popen(argv, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                             stderr=subprocess.PIPE)
    tail, total = bytearray(), 0

    def drain():
        nonlocal tail, total
        while True:
            chunk = child.stderr.read(65536)
            if not chunk:
                return
            total += len(chunk)
            tail = (tail + chunk)[-TAIL:]
            if total > stderr_limit:
                stop(child)
                return

    reader = threading.Thread(target=drain, daemon=True)
    reader.start()
    code = child.wait()
    reader.join(timeout=5)
    wall = time.perf_counter_ns() - start
    if total > stderr_limit:
        sys.stderr.write(tail.decode('utf-8', 'replace'))
        sys.stderr.write(f'\nworkload stderr exceeded {stderr_limit} bytes; stopped\n')
        sys.exit(3)
    if code != 0:
        sys.stderr.write(tail.decode('utf-8', 'replace'))
        sys.stderr.write(f'\nworkload command exited {code}\n')
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
