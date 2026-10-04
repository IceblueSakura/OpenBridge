//! Independent synthetic wire oracle for the closed Responses client attachment.
use serde_json::{Value, json};
pub fn response() -> Value {
    let mut response = crate::wire::response(2);
    response["created_at"] = json!(1);
    response["completed_at"] = json!(2);
    response["_openbridge"] = json!({"version":1,"progress":"awaiting_tool_results"});
    response["output"] = json!([
      {"type":"reasoning","id":"thought","summary":[],"status":"completed","_openbridge":{"version":1,"replay":{"format":"google-interactions-v1-thought","phase":"final","value":"synthetic-opaque"}}},
      {"type":"message","id":"owner","role":"assistant","content":[],"status":"completed"},
      {"type":"function_call","id":"call-one","call_id":"c1","name":"lookup","arguments":"{\"n\":18446744073709551616001}","status":"completed","_openbridge":{"version":1,"arguments":"json","message":"owner"}},
      {"type":"function_call","id":"call-two","call_id":"c2","name":"lookup","arguments":"{\"n\":2}","status":"completed","_openbridge":{"version":1,"arguments":"json","message":"owner"}}
    ]);
    response
}

pub fn events() -> Vec<Value> {
    events_for(response())
}
pub fn events_for(final_response: Value) -> Vec<Value> {
    let mut initial = final_response.clone();
    initial["status"] = json!("in_progress");
    initial["output"] = json!([]);
    initial["usage"] = Value::Null;
    initial["completed_at"] = Value::Null;
    initial.as_object_mut().unwrap().remove("_openbridge");
    let mut events = vec![json!({"type":"response.created","response":initial})];
    for (index, item) in final_response["output"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let mut added = item.clone();
        added["status"] = json!("in_progress");
        if item["type"] == "function_call" {
            added["arguments"] = json!("");
            added["_openbridge"]["arguments"] = json!("json_partial");
        }
        events.push(json!({"type":"response.output_item.added","output_index":index,"item":added}));
        if item["type"] == "function_call" {
            events.push(json!({"type":"response.function_call_arguments.delta","output_index":index,"item_id":item["id"],"delta":item["arguments"]}));
            events.push(json!({"type":"response.function_call_arguments.done","output_index":index,"item_id":item["id"],"arguments":item["arguments"]}));
        }
        events.push(json!({"type":"response.output_item.done","output_index":index,"item":item}));
    }
    events.push(json!({"type":"response.completed","response":final_response}));
    for (sequence, event) in events.iter_mut().enumerate() {
        event["sequence_number"] = json!(sequence);
    }
    events
}
