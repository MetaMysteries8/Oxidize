"""Mutation fixture: supplied workloads use small integer lists."""


def total_and_mark(values):
    total = sum(values)
    values.append(total)
    if total < 0:
        raise ValueError("negative total")
    return total
