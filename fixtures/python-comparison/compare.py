import sys


def observe(values):
    values.append(7)
    print("result\r\nready", end="")
    print("diagnostic\r", end="", file=sys.stderr)
    if values[0] < 0:
        raise ValueError("bad\r\nvalue")
    return {"value": (0.1 + 0.2, "leave\r\nreturn strings alone")}
