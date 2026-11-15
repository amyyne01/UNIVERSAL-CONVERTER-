//! Compares how a download request becomes a task with the `download:start`
//! handler in electron/ipc.ts (tests/golden/api.json, written by
//! scripts/rust-golden/api.mjs through the real handler registration).
use ahg_engine::api::build_task;
use ahg_engine::tier::Plan;
use serde_json::{Value, json};

#[test]
fn a_request_becomes_the_same_task_or_the_same_refusal_as_in_electron() {
    let cases: Vec<Value> = serde_json::from_str(include_str!("golden/api.json")).unwrap();
    assert_eq!(cases.len(), 6, "three settings files, two plans each");
    let mut wrong = Vec::new();
    let mut checked = 0;
    for case in &cases {
        let config = case["config"].as_object().unwrap();
        let plan: Plan = serde_json::from_value(case["plan"].clone()).unwrap();
        let cookies = case["cookiesFile"].as_str();
        for o in case["out"].as_array().unwrap() {
            let request = if o["request"] == "__undefined__" { Value::Null } else { o["request"].clone() };
            let id = o["task"]["taskId"].as_str().unwrap_or("task_unused").to_owned();
            let got = match build_task(&request, config, plan, cookies, id, o["now"].as_u64().unwrap()) {
                Ok(task) => json!({ "task": task }),
                Err(error) => json!({ "error": error }),
            };
            let expected = match o.get("error") {
                Some(error) => json!({ "error": error }),
                None => json!({ "task": o["task"] }),
            };
            if got != expected {
                let show = |v: &Value| v.to_string().chars().take(900).collect::<String>();
                wrong.push(format!(
                    "[{} / {}] {}\n  electron: {}\n  rust:     {}",
                    case["settings"],
                    case["plan"],
                    show(&o["request"]),
                    show(&expected),
                    show(&got)
                ));
            }
            checked += 1;
        }
    }
    assert!(checked > 600, "only {checked} requests checked");
    assert!(wrong.is_empty(), "{} mismatches:\n{}", wrong.len(), wrong.iter().take(8).cloned().collect::<Vec<_>>().join("\n"));
}
