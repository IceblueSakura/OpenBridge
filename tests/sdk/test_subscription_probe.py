"""Explicit OAuth activation stays in the binary; no credential discovery."""
import os
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "examples"))
from probe_support.catalog import select_bindings
from probe_support.ledger import Run
from probe_support.runtime import gateway

class SubscriptionProbeTests(unittest.TestCase):
    def test_probe_passes_only_explicit_store_alias_not_tokens(self):
        self.assertFalse({"codex", "grok"} & {row[0] for row in select_bindings()})
        with tempfile.TemporaryDirectory() as directory:
            run = Run.create(Path(directory) / "run", providers="codex", limit=1)
            class Stop(Exception):
                pass
            def spawn(command, **kwargs):
                env = kwargs["env"]
                self.assertEqual(env["OPENBRIDGE_CREDENTIAL_STORE"], "/synthetic/owned/store")
                self.assertEqual(env["OPENBRIDGE_CODEX_ACCOUNT"], "chosen")
                self.assertNotIn("OPENBRIDGE_GROK_ACCOUNT", env)
                self.assertNotIn("OPENBRIDGE_CODEX_API_KEY", env)
                raise Stop()
            with patch.dict(os.environ, {
                "OPENBRIDGE_PROBE_LIVE": "1",
                "OPENBRIDGE_CREDENTIAL_STORE": "/synthetic/owned/store",
                "OPENBRIDGE_CODEX_ACCOUNT": "chosen",
            }, clear=True), patch("probe_support.runtime.subprocess.Popen", side_effect=spawn), patch(
                "pathlib.Path.read_text", side_effect=AssertionError("no private config reads")
            ):
                with self.assertRaises(Stop):
                    with gateway(run):
                        self.fail("must not start a real binary")
