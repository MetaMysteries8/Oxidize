"""Small intentionally slow component with a bounded public contract."""


def count_primes(limit: int) -> int:
    """Count primes strictly below limit, for integer limits in 0..1,000,000."""
    if type(limit) is not int:
        raise TypeError("limit must be an integer")
    if limit < 0:
        raise ValueError("limit must be non-negative")
    if limit > 1_000_000:
        raise ValueError("limit too large")
    count = 0
    for number in range(2, limit):
        prime = True
        divisor = 2
        while divisor * divisor <= number:
            if number % divisor == 0:
                prime = False
                break
            divisor += 1
        if prime:
            count += 1
    return count
