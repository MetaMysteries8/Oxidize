import math
import unittest
from compute import count_primes


class ComputeTests(unittest.TestCase):
    def test_known_counts(self):
        for limit, expected in [(0, 0), (1, 0), (2, 0), (3, 1), (10, 4), (100, 25), (10000, 1229)]:
            with self.subTest(limit=limit):
                self.assertEqual(count_primes(limit), expected)

    def test_against_independent_reference(self):
        for limit in range(200):
            expected = sum(all(n % d for d in range(2, math.isqrt(n) + 1)) for n in range(2, limit))
            self.assertEqual(count_primes(limit), expected)

    def test_errors(self):
        for limit in [-1, -(10 ** 100)]:
            with self.assertRaisesRegex(ValueError, '^limit must be non-negative$'):
                count_primes(limit)
        for limit in [1000001, 10 ** 100]:
            with self.assertRaisesRegex(ValueError, '^limit too large$'):
                count_primes(limit)
        for limit in [True, False, 1.2, '10', None]:
            with self.assertRaisesRegex(TypeError, '^limit must be an integer$'):
                count_primes(limit)


if __name__ == '__main__':
    unittest.main()
