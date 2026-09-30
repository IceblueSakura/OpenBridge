"""Independent offline boundary fixtures; never load workspace credentials."""

import concurrent.futures
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "examples"))
from probe_support.ledger import Run
from probe_support.checks import ProbeFailure
from probe_support.wire import Wire


def reserve_worker(path, number):
    try:
        return Run(path).reserve("nemotron-3-super", f"case-{number}", 8)
    except ProbeFailure:
        return None


class ProbeCoreTests(unittest.TestCase):
    def test_model_subset_is_enforced_by_persistent_reservations(self):
        from probe_support.scenarios import plan_groups

        with tempfile.TemporaryDirectory() as temp:
            run = Run.create(
                Path(temp) / "run",
                providers="xiaomi,longcat",
                models=["mimo-v2.6-flash", "longcat-2.5-preview"],
                limit=72,
            )
            run = Run(run.directory)
            with self.assertRaises(ProbeFailure):
                run.reserve("mimo-v2.6-pro", "unselected", 8)
            groups = plan_groups(
                run, run.plan["models"],
                cases=("text", "json", "tool", "history", "length", "cancel"),
            )
            self.assertEqual(sum(group[5] for group in groups), 72)
            run.reserve("mimo-v2.6-flash", "selected", 8)

    def test_image_matrix_is_eight_bounded_requests_with_independent_pixels(self):
        import base64
        import struct
        import zlib
        from probe_support.scenarios import plan_groups
        from probe_support.images import image_history

        with tempfile.TemporaryDirectory() as temp:
            run = Run.create(Path(temp) / "run", providers="deepseek,xiaomi",
                models=["deepseek-flash", "mimo-v2.6-flash"], limit=8, tokens=512)
            groups = plan_groups(run, run.plan["models"], cases=("image",), effort="none")
            self.assertEqual(len(groups), 8)
            self.assertEqual(sum(group[5] for group in groups), 8)
            self.assertTrue(all(group[6] == 512 for group in groups))
        for protocol in ("chat", "responses"):
            content = image_history(protocol)[0]["content"]
            self.assertEqual(len(content), 4)
            for index, rgb in ((1, bytes((255, 0, 0))), (3, bytes((0, 0, 255)))):
                image = content[index]
                url = image["image_url"]["url"] if protocol == "chat" else image["image_url"]
                png = base64.b64decode(url.split(",", 1)[1], validate=True)
                self.assertEqual(png[:8], b"\x89PNG\r\n\x1a\n")
                offset, pixels = 8, None
                while offset < len(png):
                    size = struct.unpack(">I", png[offset:offset+4])[0]
                    kind = png[offset+4:offset+8]
                    data = png[offset+8:offset+8+size]
                    crc = struct.unpack(">I", png[offset+8+size:offset+12+size])[0]
                    self.assertEqual(crc, zlib.crc32(kind+data) & 0xFFFFFFFF)
                    if kind == b"IHDR":
                        self.assertEqual(struct.unpack(">IIBBBBB", data), (192,192,8,2,0,0,0))
                    if kind == b"IDAT":
                        pixels = zlib.decompress(data)
                    offset += size + 12
                self.assertEqual(pixels, (b"\x00"+rgb*192)*192)

    def test_visual_reasoning_matrix_budget_and_pixel_counts_have_independent_oracles(self):
        import base64
        import struct
        import zlib
        from probe_support.scenarios import plan_groups, expect_visual_math
        from probe_support.images import visual_math_history
        with tempfile.TemporaryDirectory() as temp:
            run = Run.create(Path(temp)/"run",providers="xiaomi",
                models=["mimo-v2.6-pro"],limit=24,tokens=2048)
            groups = [group for effort in ("none","minimal","medium")
                for group in plan_groups(run,run.plan["models"],cases=("image","image_math"),effort=effort)]
            self.assertEqual(sum(group[5] for group in groups),24)
            self.assertEqual(len(set(group[4] for group in groups)),24)
            self.assertTrue(all(group[6]==(512 if group[3]=="image" else 2048) for group in groups))
        for protocol in ("chat","responses"):
            content = visual_math_history(protocol)[0]["content"]
            self.assertEqual(len(content),4)
            counts = []
            for index in (1,3):
                url = content[index]["image_url"]
                if protocol=="chat": url=url["url"]
                png = base64.b64decode(url.split(",",1)[1],validate=True)
                offset, data = 8, bytearray()
                while offset<len(png):
                    size = struct.unpack(">I",png[offset:offset+4])[0]
                    if png[offset+4:offset+8]==b"IDAT": data.extend(png[offset+8:offset+8+size])
                    offset += size+12
                pixels = zlib.decompress(data)
                rgb = [pixels[row*(1+192*3)+1:row*(1+192*3)+1+192*3] for row in range(192)]
                colors = [rgb[y*64+32][(x*64+32)*3:(x*64+32)*3+3] for y in range(3) for x in range(3)]
                counts.append((colors.count(bytes((255,0,0))),colors.count(bytes((0,0,255)))))
            self.assertEqual(counts,[(5,4),(3,6)])
            answer=counts[0][0]*counts[1][1]-counts[0][1]*counts[1][0]
            self.assertEqual(answer,18)
            expect_visual_math(json.dumps({"answer":answer}),[],[])
        for text,code in (('{"answer":17}','visual_math_value'),
            ('{"answer":"18"}','visual_math_format'),('{"answer":18,"extra":0}','visual_math_format'),
            ('{"answer":17,"answer":18}','visual_math_format')):
            with self.assertRaises(ProbeFailure) as raised: expect_visual_math(text,[],[])
            self.assertEqual(raised.exception.code,code)

    def test_plan_cannot_be_mutated_in_memory_and_expiry_preserves_readback(self):
        with tempfile.TemporaryDirectory() as temp:
            run = Run.create(Path(temp) / "run", limit=1)
            view = run.plan
            view["limit"] = 99
            view["models"].append("kimi-k3")
            self.assertEqual(run.plan["limit"], 1)
            self.assertNotIn("kimi-k3", run.plan["models"])
            identity = run.reserve("nemotron-3-super", "first", 8)
            with patch(
                "probe_support.ledger.time.time", return_value=run.plan["expires"] + 1
            ):
                run.finish(identity, "passed", {"content_ok": True})
                self.assertEqual(Run(run.directory).snapshot()[0]["state"], "passed")
                with self.assertRaises(ProbeFailure):
                    run.reserve("nemotron-3-super", "later", 8)

    def test_cross_process_reservations_crash_and_changed_plan_fail_closed(self):
        with tempfile.TemporaryDirectory() as temp:
            run = Run.create(Path(temp) / "run", limit=3)
            with concurrent.futures.ProcessPoolExecutor(max_workers=4) as pool:
                ids = list(pool.map(reserve_worker, [str(run.directory)] * 8, range(8)))
            self.assertEqual(sum(x is not None for x in ids), 3)
            self.assertEqual(len(Run(run.directory).snapshot()), 3)
            with self.assertRaises(ProbeFailure):
                run.reserve("nemotron-3-super", "another", 8)
            path = run.directory / "plan.json"
            plan = json.loads(path.read_text())
            plan["limit"] = 20
            path.write_text(json.dumps(plan))
            with self.assertRaises(ProbeFailure):
                Run(run.directory)

    def test_pending_is_not_replayed_and_reports_cannot_leak_payloads(self):
        with tempfile.TemporaryDirectory() as temp:
            run = Run.create(Path(temp) / "run", limit=3, continue_oracle=True)
            identity = run.reserve("nemotron-3-super", "first", 8)
            with self.assertRaises(ProbeFailure):
                Run(run.directory).reserve("nemotron-3-super", "first", 8)
            with self.assertRaises(RuntimeError):
                run.finish(identity, "failed", {"body": "synthetic-private"})
            run.dispatched(identity)
            run.finish(identity, "oracle_failed", {"failure": "oracle"})
            identity = run.reserve("nemotron-3-super", "second", 8)
            run.finish(identity, "failed", {"http": 429, "retry_after_seconds": 7})
            with self.assertRaises(ProbeFailure):
                run.reserve("nemotron-3-super", "third", 8)

    def test_sse_requires_unique_finish_done_and_no_data_after_it(self):
        finish = 'data: {"choices":[{"index":0,"delta":{"content":"pong"},"finish_reason":"stop"}]}\n\n'
        wire = Wire("chat", True)
        for byte in (finish + "data: [DONE]\n\n").encode():
            wire.push(bytes([byte]))
        wire.finish()
        self.assertTrue(wire.closed)
        self.assertEqual(wire.text, "pong")
        for raw in (
            finish,
            finish + finish + "data: [DONE]\n\n",
            "data: [DONE]\n\n",
            finish + "data: [DONE]\n\ndata: {}\n\n",
        ):
            with self.assertRaises(ProbeFailure):
                value = Wire("chat", True)
                value.push(raw.encode())
                value.finish()
            with self.assertRaises(ProbeFailure):
                value.finish()
            self.assertFalse(value.closed)
        with self.assertRaises(ProbeFailure):
            Wire("chat", True, limit=8).push(b"x" * 9)

    def test_checks_survive_optimized_python(self):
        root = Path(__file__).resolve().parents[2]
        code = "import sys;sys.path.insert(0,'examples');from probe_support.scenarios import expect_text;expect_text('42')('142',[],[])"
        result = subprocess.run(
            [sys.executable, "-O", "-c", code], cwd=root, capture_output=True
        )
        self.assertNotEqual(result.returncode, 0)


if __name__ == "__main__":
    unittest.main()
