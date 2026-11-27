//! Property tests for the MCP server's request handling (`src/mcp.rs`): any
//! JSON gets a well-formed answer or none, a client string never reaches
//! priorart's command line as a flag or as a writing or networked command,
//! and arbitrary bytes on stdin never panic the reader or the loop.

use priorart::mcp::{self, Framing, Incoming, Policy, Ran, INVALID_PARAMS};
use proptest::prelude::*;
use serde_json::{json, Value};

const TOOLS: &[&str] = &[
    "check",
    "candidates",
    "anchors_resolve",
    "anchors_index",
    "taxonomy_classify",
    "ledger_validate",
    "vocab_list",
    "vocab_show",
    "explain",
    "rules",
    "rejections_summary",
];

const KEYS: &[&str] = &[
    "finding",
    "dossier",
    "top_k",
    "fail_on_excluded",
    "fail_on_unpayable",
    "corpus",
    "repo",
    "index",
    "repo_id",
    "commit",
    "exclude",
    "ranker",
    "alarm",
    "prose_anchors",
    "text_file",
    "files",
    "ledgers",
    "strict",
    "example_mode",
    "name",
    "id",
    "prepare",
    "out",
    "force",
];

fn arb_json() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(|n| json!(n)),
        (0u64..2000).prop_map(|n| json!(n)),
        any::<f64>()
            .prop_filter("finite", |f| f.is_finite())
            .prop_map(|f| json!(f)),
        ".{0,12}".prop_map(Value::String),
        prop_oneof![
            Just("--prepare".to_string()),
            Just("-o".to_string()),
            Just("fetch".to_string()),
            Just("tier-shapes".to_string()),
            Just("a/b".to_string()),
            Just("0000000".to_string()),
            Just("f.md".to_string()),
            Just("initialize".to_string()),
            Just("tools/call".to_string()),
            Just("2.0".to_string()),
        ]
        .prop_map(Value::String),
        prop::sample::select(TOOLS).prop_map(|s| Value::String(s.into())),
    ];
    leaf.prop_recursive(4, 48, 6, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..5).prop_map(Value::Array),
            prop::collection::btree_map(
                prop_oneof![
                    Just("jsonrpc".to_string()),
                    Just("id".to_string()),
                    Just("method".to_string()),
                    Just("params".to_string()),
                    Just("arguments".to_string()),
                    prop::sample::select(KEYS).prop_map(String::from),
                    "[a-z_]{1,8}",
                ],
                inner,
                0..6
            )
            .prop_map(|m| Value::Object(m.into_iter().collect())),
        ]
    })
}

/// Arguments built from the tools' own keys, with adversarial values.
fn arb_arguments() -> impl Strategy<Value = Value> {
    let value = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        (0u64..1100).prop_map(|n| json!(n)),
        "\\PC{0,16}".prop_map(Value::String),
        prop_oneof![
            Just("--prepare=x"),
            Just("-h"),
            Just(" --out"),
            Just("f.md"),
            Just("a/b"),
            Just("deadbeef"),
            Just("tier-shapes"),
            Just("G1"),
        ]
        .prop_map(|s| Value::String(s.into())),
        prop::collection::vec(
            prop_oneof![
                "\\PC{0,10}".prop_map(Value::String),
                Just(Value::String("--prepare".into())),
                Just(Value::String("r".into())),
                Just(json!(1)),
            ],
            0..4
        )
        .prop_map(Value::Array),
    ];
    prop::collection::btree_map(
        prop::sample::select(KEYS).prop_map(String::from),
        value,
        0..6,
    )
    .prop_map(|m| Value::Object(m.into_iter().collect()))
}

fn arb_call() -> impl Strategy<Value = Value> {
    (
        prop_oneof![
            prop::sample::select(TOOLS).prop_map(String::from),
            Just("fetch".to_string()),
            Just("build".to_string()),
            Just("init".to_string()),
            ".{0,8}",
        ],
        prop_oneof![arb_arguments(), arb_json()],
        any::<u32>(),
    )
        .prop_map(|(name, arguments, id)| {
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "tools/call",
                "params": { "name": name, "arguments": arguments }
            })
        })
}

fn record(calls: &mut Vec<Vec<String>>) -> impl FnMut(&[String]) -> Result<Ran, String> + '_ {
    move |args: &[String]| {
        calls.push(args.to_vec());
        Ok(Ran {
            code: 0,
            stdout: "{}".into(),
            stderr: String::new(),
        })
    }
}

/// The commands a tool may start with (first two tokens).
const COMMANDS: &[&[&str]] = &[
    &["check"],
    &["candidates"],
    &["anchors", "resolve"],
    &["anchors", "index"],
    &["taxonomy", "classify"],
    &["ledger", "validate"],
    &["vocab", "list"],
    &["vocab", "show"],
    &["explain"],
    &["rules"],
    &["rejections", "summary"],
];
/// Flags that stand alone.
const SWITCHES: &[&str] = &[
    "--json",
    "--fail-on-excluded",
    "--fail-on-unpayable",
    "--prose-anchors",
    "--strict",
    "--example-mode",
];
/// Flags followed by one client value.
const TAKES_VALUE: &[&str] = &[
    "--dossier",
    "--top-k",
    "--corpus",
    "--repo",
    "--index",
    "--repo-id",
    "--commit",
    "--exclude",
    "--ranker",
    "--alarm",
    "--text-file",
];

fn assert_argv_is_safe(argv: &[String]) -> Result<(), TestCaseError> {
    let command = COMMANDS.iter().find(|c| {
        argv.len() >= c.len() && argv[..c.len()].iter().zip(c.iter()).all(|(a, b)| a == b)
    });
    prop_assert!(command.is_some(), "not a read-only command: {argv:?}");
    for a in argv {
        prop_assert!(
            ![
                "fetch",
                "build",
                "init",
                "bench",
                "--prepare",
                "--out",
                "--force"
            ]
            .contains(&a.as_str()),
            "{argv:?}"
        );
        prop_assert!(
            !a.starts_with("--prepare") && !a.starts_with("--out"),
            "{argv:?}"
        );
        prop_assert!(
            !a.contains('\0') && !a.contains('\n') && !a.contains('\r'),
            "{argv:?}"
        );
        prop_assert!(a.len() <= mcp::MAX_ARG_BYTES, "{argv:?}");
    }
    let mut i = command.unwrap().len();
    while i < argv.len() {
        let a = &argv[i];
        if a == "--" {
            // Positional values only, none of them flag-like.
            for v in &argv[i + 1..] {
                prop_assert!(!v.starts_with('-'), "{argv:?}");
            }
            prop_assert!(argv.len() > i + 1, "nothing after --: {argv:?}");
            break;
        }
        if TAKES_VALUE.contains(&a.as_str()) {
            prop_assert!(i + 1 < argv.len(), "{argv:?}");
            let v = &argv[i + 1];
            prop_assert!(!v.starts_with('-'), "a flag value is a flag: {argv:?}");
            if a == "--top-k" {
                let k: u64 = v.parse().unwrap();
                prop_assert!((1..=mcp::MAX_TOP_K).contains(&k), "{argv:?}");
            }
            if a == "--commit" {
                prop_assert!(
                    v.len() >= 7 && v.bytes().all(|b| b.is_ascii_hexdigit()),
                    "{argv:?}"
                );
            }
            i += 2;
            continue;
        }
        prop_assert!(
            SWITCHES.contains(&a.as_str()),
            "a client token outside a value slot: {argv:?}"
        );
        i += 1;
    }
    Ok(())
}

fn has_float(v: &Value) -> bool {
    match v {
        Value::Number(n) => n.is_f64(),
        Value::Array(a) => a.iter().any(has_float),
        Value::Object(o) => o.values().any(has_float),
        _ => false,
    }
}

fn call(name: &str, arguments: Value) -> (Value, Vec<Vec<String>>) {
    let mut calls = Vec::new();
    let out = {
        let mut run = record(&mut calls);
        mcp::dispatch(
            &json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":name,"arguments":arguments}}),
            &mut run,
        )
        .unwrap()
    };
    (out, calls)
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 512, ..ProptestConfig::default() })]

    #[test]
    fn any_json_gets_a_well_formed_answer_or_none(msg in arb_json()) {
        let mut calls = Vec::new();
        let mut run = record(&mut calls);
        if let Some(out) = mcp::dispatch(&msg, &mut run) {
            prop_assert_eq!(&out["jsonrpc"], "2.0");
            let has_result = out.get("result").is_some();
            let has_error = out.get("error").is_some();
            prop_assert!(has_result != has_error, "{out}");
            if has_error {
                prop_assert!(out["error"]["code"].is_i64(), "{out}");
                prop_assert!(out["error"]["message"].is_string(), "{out}");
            }
            if !out["id"].is_null() {
                prop_assert_eq!(&out["id"], &msg["id"]);
            }
        } else {
            prop_assert!(msg.get("id").is_none(), "only a notification goes unanswered: {msg}");
        }
    }

    #[test]
    fn a_tool_call_never_reaches_a_flag_slot_or_a_writing_command(msg in arb_call()) {
        let mut calls = Vec::new();
        let out = {
            let mut run = record(&mut calls);
            mcp::dispatch(&msg, &mut run).unwrap()
        };
        prop_assert!(calls.len() <= 1);
        if out.get("error").is_some() {
            prop_assert!(calls.is_empty(), "a refused call ran: {msg}");
        } else {
            prop_assert_eq!(calls.len(), 1, "{}", out);
        }
        for argv in &calls {
            assert_argv_is_safe(argv)?;
        }
    }

    #[test]
    fn check_runs_exactly_when_the_finding_is_a_path(path in "\\PC{0,40}") {
        let (out, calls) = call("check", json!({ "finding": path }));
        let t = path.trim();
        let valid = !t.is_empty() && !t.starts_with('-') && !t.contains(['\0', '\n', '\r']);
        prop_assert_eq!(calls.len(), usize::from(valid), "{}", out);
        if valid {
            prop_assert_eq!(&calls[0], &vec!["check".to_string(), "--json".into(), "--".into(), t.to_string()]);
        } else {
            prop_assert_eq!(&out["error"]["code"], INVALID_PARAMS);
        }
    }

    #[test]
    fn explain_runs_only_a_bounded_non_flag_id(id in "\\PC{0,20}|-?[A-Za-z0-9_-]{0,300}") {
        let (_, calls) = call("explain", json!({ "id": id }));
        let t = id.trim();
        let valid = !t.is_empty() && t.len() <= 256 && !t.starts_with('-') && !t.chars().any(char::is_control);
        prop_assert_eq!(calls.len(), usize::from(valid));
        if valid {
            prop_assert_eq!(&calls[0], &vec!["explain".to_string(), "--".into(), t.to_string()]);
        }
    }

    #[test]
    fn top_k_is_passed_exactly_when_in_range(k in any::<i64>()) {
        let (_, calls) = call("check", json!({ "finding": "f.md", "top_k": k }));
        let valid = (1..=1000).contains(&k);
        prop_assert_eq!(calls.len(), usize::from(valid));
        if valid {
            prop_assert!(calls[0].windows(2).any(|w| w[0] == "--top-k" && w[1] == k.to_string()));
        }
    }

    #[test]
    fn commit_is_passed_exactly_when_it_is_7_to_64_hex(c in "[0-9a-fA-Fg-z]{0,70}") {
        let (_, calls) = call("anchors_index", json!({ "repo": "r", "commit": c }));
        let valid = (7..=64).contains(&c.len()) && c.bytes().all(|b| b.is_ascii_hexdigit());
        prop_assert_eq!(calls.len(), usize::from(valid));
    }

    #[test]
    fn an_argument_outside_the_schema_is_refused(tool in prop::sample::select(TOOLS), key in "[a-z_]{1,12}") {
        let listed = mcp::tools().into_iter().find(|t| t["name"] == tool).unwrap();
        prop_assume!(listed["inputSchema"]["properties"].get(key.as_str()).is_none());
        let (out, calls) = call(tool, json!({ key: "x" }));
        prop_assert_eq!(&out["error"]["code"], INVALID_PARAMS);
        prop_assert!(calls.is_empty());
    }

    #[test]
    fn a_line_round_trips_any_message(msg in arb_json()) {
        // serde_json without `float_roundtrip` may read a float back one ulp
        // off; the server never round-trips a float (ids are integers, tool
        // text is a string), so floats are out of this property.
        prop_assume!(!has_float(&msg));
        let mut buf = Vec::new();
        mcp::write_line(&mut buf, &msg).unwrap();
        prop_assert_eq!(buf.iter().filter(|b| **b == b'\n').count(), 1);
        let got = mcp::read_message(&mut std::io::Cursor::new(buf)).unwrap().unwrap();
        match got {
            (Incoming::Message(v), Framing::Line) => {
                prop_assert_eq!(v, msg);
            }
            other => prop_assert!(false, "{other:?}"),
        }
    }

    #[test]
    fn arbitrary_bytes_never_panic_the_reader(bytes in prop::collection::vec(any::<u8>(), 0..4096)) {
        let mut input = std::io::Cursor::new(bytes);
        for _ in 0..10_000 {
            match mcp::read_message(&mut input) {
                Ok(Some(_)) => continue,
                Ok(None) | Err(_) => break,
            }
        }
    }

    #[test]
    fn arbitrary_bytes_never_panic_the_server_loop_and_every_line_answer_is_json(
        bytes in prop::collection::vec(prop_oneof![any::<u8>(), Just(b'\n'), Just(b'{'), Just(b'}')], 0..2048)
    ) {
        let mut out = Vec::new();
        let mut calls = Vec::new();
        {
            let mut run = record(&mut calls);
            let _ = mcp::serve(&mut std::io::Cursor::new(bytes.clone()), &mut out, &Policy::default(), &mut run);
        }
        let lower = String::from_utf8_lossy(&bytes).to_ascii_lowercase();
        if !lower.contains("content-length") {
            for line in String::from_utf8(out).unwrap().lines() {
                let v: Value = serde_json::from_str(line).unwrap();
                prop_assert_eq!(&v["jsonrpc"], "2.0");
            }
        }
    }

    #[test]
    fn a_framed_message_of_any_length_under_the_cap_round_trips(s in "\\PC{0,200}", id in any::<u32>()) {
        let msg = json!({"jsonrpc":"2.0","id":id,"method":"ping","params":{"s":s}});
        let mut buf = Vec::new();
        mcp::write_frame(&mut buf, &msg).unwrap();
        let got = mcp::read_message(&mut std::io::Cursor::new(buf)).unwrap().unwrap();
        prop_assert_eq!(got, (Incoming::Message(msg), Framing::ContentLength));
    }
}
