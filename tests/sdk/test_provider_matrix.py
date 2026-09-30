"""Live selections fail before reading credentials or touching a socket."""

from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "examples"))
from probe_support.catalog import select_bindings


class SelectionTests(unittest.TestCase):
    def test_paused_unknown_and_duplicate_selections_fail_closed(self):
        rows = select_bindings()
        self.assertEqual(rows[0][0], "nvidia")
        self.assertNotIn("kimi", [row[0] for row in rows])
        for selection in ("kimi", "nvidia,kimi", "", "unknown", "nvidia,nvidia"):
            with self.assertRaises(RuntimeError):
                select_bindings(selection)
        self.assertEqual(
            [row[0] for row in select_bindings("zhipu,nvidia")], ["zhipu", "nvidia"]
        )


if __name__ == "__main__":
    unittest.main()
