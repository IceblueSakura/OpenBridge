"""Persist pinned SDK client attachments and return complete history to loopback."""
import json
import sys
from sdk_support import client_for, check

def run(base_url: str, stream: bool):
    client = client_for(base_url)
    tools = [{"type":"function", "name":"lookup", "parameters":{"type":"object","properties":{"n":{"type":"integer"}},"required":["n"],"additionalProperties":False}, "strict":True}]
    params = dict(model="fixture-model", input="initial", tools=tools, store=False)
    try:
        if stream:
            with client.responses.stream(**params) as events:
                for _ in events:
                    pass
                result = events.get_final_response()
        else:
            result = client.responses.parse(**params)
        check(result.model_dump(exclude_none=True)["_openbridge"]["progress"] == "awaiting_tool_results")
        root = result.model_dump(exclude_none=True)["_openbridge"]
        check(root["usage"][0]["scope"]["item"] == "call-one")
        check("output_tokens" not in root["usage"][0])
        check(root["usage"][1]["reasoning_tokens"] == 2)
        dumped = [item.model_dump(exclude_none=True) for item in result.output]
        check(dumped[0]["_openbridge"]["replay"]["value"] == "synthetic-opaque")
        check("encrypted_content" not in dumped[0])
        check(dumped[2]["_openbridge"]["message"] == dumped[1]["id"])
        check(json.loads(dumped[2]["arguments"])["n"] == 18446744073709551616001)
        check(dumped[2]["parsed_arguments"]["n"] == 18446744073709551616001)
        # JSON persistence must be part of the boundary, not just an in-memory model.
        dumped = json.loads(json.dumps(dumped))
        history = [{"role":"user","content":"initial"}, *dumped,
                   {"type":"function_call_output","call_id":"c1","output":'{"ok":true}',"_openbridge":{"version":1,"output":"json","execution":{"status":"succeeded"}}},
                   {"type":"function_call_output","call_id":"c2","output":'{"detail":2}',"_openbridge":{"version":1,"output":"json","execution":{"status":"failed","code":"synthetic"}}}]
        final = client.responses.create(model="fixture-model",input=history,store=False)
        check(final.status == "completed")
        return {"turns":2,"stream":stream}
    finally:
        client.close()

if __name__ == "__main__":
    if len(sys.argv)!=3 or sys.argv[2] not in ("json","sse"):
        raise SystemExit("usage: client_carrier_loop.py LOOPBACK_BASE_URL json|sse")
    try:
        print(json.dumps(run(sys.argv[1], sys.argv[2]=="sse")))
    except Exception as exc:
        cause=exc.__cause__
        errors=([{"loc":list(entry["loc"]),"type":entry["type"]} for entry in cause.errors()]
                if cause is not None and hasattr(cause,"errors") else [])
        print(json.dumps({"error":type(exc).__name__,"status":getattr(exc,"status_code",None),"validation":errors[:8]}),file=sys.stderr)
        raise SystemExit(1) from None
