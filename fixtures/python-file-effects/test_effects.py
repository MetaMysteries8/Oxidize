import unittest

from effects import record


class ReturnContract(unittest.TestCase):
    def test_returns_and_error(self):
        # Deliberately checks no file content: differential effects must catch it.
        for action in ("create", "change", "delete"):
            self.assertEqual(record(action), 1)
        with self.assertRaisesRegex(ValueError, "after write"):
            record("error")
