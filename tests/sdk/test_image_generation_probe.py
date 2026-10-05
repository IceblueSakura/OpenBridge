"""Independent image probe limits and raster oracles; synthetic bytes only."""
import base64
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0,str(Path(__file__).resolve().parents[2]/"examples"))
from probe_support.ledger import Run
from probe_support.checks import ProbeFailure
from probe_support.images import color_png

class ImageGenerationProbeTests(unittest.TestCase):
    def test_image_plan_uses_request_and_image_budget_not_fictitious_tokens(self):
        with tempfile.TemporaryDirectory() as directory:
            run=Run.create(Path(directory)/"run",providers="openrouter",models=["gpt-image-2.5-flare"],limit=3,tokens=None,task="images")
            self.assertEqual(run.plan["version"],2)
            self.assertIsNone(run.plan["tokens"])
            self.assertEqual(run.plan["images_per_request"],1)
            run.register([("gpt-image-2.5-flare","images:minimal",None)])
            attempt=run.reserve("gpt-image-2.5-flare","images:minimal",None)
            run.dispatched(attempt)
            run.finish(attempt,"passed",{"image_decoded":True,"image_count":1})
            self.assertEqual(run.snapshot()[0]["images"],1)
            with self.assertRaises(ProbeFailure):run.reserve("gpt-image-2.5-flare","tokens",2048)
            with self.assertRaises(ProbeFailure):run.reserve("gpt-6-luna","wrong-task",None)
            with self.assertRaises(ProbeFailure):run.reserve("gpt-image-2.5-flare","images:minimal",None)
    def test_matrix_is_three_fixed_single_image_cases(self):
        from probe_support.image_generation import plan_groups
        with tempfile.TemporaryDirectory() as directory:
            run=Run.create(Path(directory)/"run",providers="openrouter",models=["gpt-image-2.5-flare"],limit=3,tokens=None,task="images")
            groups=plan_groups(run,run.plan["models"])
            self.assertEqual(len(groups),3)
            self.assertTrue(all(row[5:]==(1,None) for row in groups))
            with self.assertRaises(ProbeFailure):plan_groups(run,run.plan["models"],delivery="sse")
    def test_png_oracle_checks_pixels_checksums_and_bounded_inflation(self):
        from probe_support.image_generation import inspect_png
        blue=base64.b64decode(color_png((0,0,255)).split(",",1)[1])
        facts=inspect_png(blue,"blue")
        self.assertEqual((facts["image_width"],facts["image_height"]),(192,192))
        self.assertTrue(facts["content_ok"])
        self.assertFalse(inspect_png(blue,"red")["content_ok"])
        for invalid in (b"AQID",blue[:-1],blue+b"trailing",blue[:50]+bytes([blue[50]^1])+blue[51:]):
            with self.assertRaises(ProbeFailure):inspect_png(invalid,"blue")
    def test_all_png_filters_and_inflation_limit_have_independent_fixtures(self):
        import struct, zlib
        from probe_support.image_generation import inspect_png
        def chunk(kind,data):
            return struct.pack(">I",len(data))+kind+data+struct.pack(">I",zlib.crc32(kind+data)&0xffffffff)
        def png(raw):
            return b"\x89PNG\r\n\x1a\n"+chunk(b"IHDR",struct.pack(">IIBBBBB",2,2,8,2,0,0,0))+chunk(b"IDAT",zlib.compress(raw))+chunk(b"IEND",b"")
        blue=bytes([0,0,255,0,0,255])
        rows={0:(blue,blue),1:(bytes([0,0,255,0,0,0]),)*2,2:(blue,bytes(6)),
              3:(bytes([0,0,255,0,0,128]),bytes([0,0,128,0,0,0])),4:(bytes([0,0,255,0,0,0]),bytes(6))}
        for kind,(a,b) in rows.items():
            self.assertTrue(inspect_png(png(bytes([kind])+a+bytes([kind])+b),"blue")["content_ok"])
        with self.assertRaises(ProbeFailure):inspect_png(png(bytes(1_000_000)),"blue")
    def test_pinned_sdk_matrix_reaches_only_three_reserved_synthetic_sends(self):
        import contextlib, httpx2
        from openai import OpenAI, DefaultHttpxClient
        from probe_support.runtime import ProbeClient
        from probe_support.image_generation import matrix
        bodies=[]
        def respond(request,**kwargs):
            body=json.loads(request.content)
            bodies.append(body)
            color=(255,0,0) if len(bodies)==2 else (0,0,255)
            wire=json.dumps({"created":1,"data":[{"b64_json":color_png(color).split(",",1)[1]}],"output_format":"png"}).encode()
            return httpx2.Response(200,headers={"content-type":"application/json"},stream=httpx2.ByteStream(wire),request=request)
        @contextlib.contextmanager
        def session(run,models):
            transport=ProbeClient("http://127.0.0.1:12345",run)
            with OpenAI(base_url=transport.origin+"/v1",api_key="synthetic-client",max_retries=0,http_client=transport) as client:
                yield client,transport
        with tempfile.TemporaryDirectory() as directory:
            run=Run.create(Path(directory)/"run",providers="openrouter",models=["gpt-image-2.5-flare"],limit=3,tokens=None,task="images")
            with patch("probe_support.image_generation.session",session),patch.object(DefaultHttpxClient,"send",side_effect=respond) as send:
                self.assertTrue(matrix(run,run.plan["models"]))
                self.assertEqual(send.call_count,3)
            self.assertEqual(set(bodies[0]),{"model","prompt"})
            self.assertIs(bodies[1]["stream"],False)
            self.assertIsNone(bodies[2]["output_format"])
            self.assertTrue(all(row["state"]=="passed" and row["tokens"] is None for row in run.snapshot()))
    def test_transport_rejects_unplanned_image_controls_before_reservation(self):
        from probe_support.runtime import ProbeClient
        import httpx2
        with tempfile.TemporaryDirectory() as directory:
            run=Run.create(Path(directory)/"run",providers="openrouter",models=["gpt-image-2.5-flare"],limit=1,tokens=None,task="images")
            with ProbeClient("http://127.0.0.1:12345",run) as client:
                client.prepare("gpt-image-2.5-flare","images","bad","synthetic")
                for patch_body in ({"n":2},{"stream":True},{"max_tokens":1},{"output_format":"png"},{"prompt":"different"}):
                    body={"model":"gpt-image-2.5-flare","prompt":"synthetic",**patch_body}
                    with self.assertRaises(ProbeFailure):client.send(httpx2.Request("POST","http://127.0.0.1:12345/v1/images/generations",json=body))
                self.assertEqual(run.snapshot(),[])

if __name__=="__main__":unittest.main()
