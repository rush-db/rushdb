//! Data directories written by every published release open on this one.
//!
//! `tests/fixtures/upgrade/<version>/` holds what `rushdb/rushdb:<version>`
//! left after the workload of `workload.json` and a clean stop, in each
//! tenancy mode, with the answers that release gave to the workload's reads
//! (`scripts/upgrade-fixtures.py regenerate` rewrites them from the published
//! images). This build must start on each one, report ready, answer every
//! read as recorded (a release may add response fields, never drop or change
//! one), take a write and keep it across a restart. CI's `upgrade-matrix` job
//! runs the same check on the release binary with deployment flags.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use rushdb_server::http::new_router;
use rushdb_server::ServerConfig;
use serde_json::{json, Value};

const MODES: [&str; 2] = ["single", "multitenant"];
/// The tenants written in multitenant mode: the first gets the workload.
const TENANTS: [&str; 2] = ["alpha", "beta"];
const TOKEN: &str = "upgrade-fixture-token";

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/upgrade")
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap())
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// Every release with a fixture, oldest first.
fn releases() -> Vec<String> {
    let mut releases: Vec<(Vec<u64>, String)> = std::fs::read_dir(fixtures())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_dir())
        .map(|path| {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let key = name.split('.').map(|part| part.parse().unwrap()).collect();
            (key, name)
        })
        .collect();
    releases.sort();
    releases.into_iter().map(|(_, name)| name).collect()
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// HTTP/1.1 body, chunked or not.
fn body(raw: &str) -> String {
    let (head, rest) = raw.split_once("\r\n\r\n").expect("an HTTP response");
    if !head
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        return rest.to_string();
    }
    let mut out = String::new();
    let mut rest = rest;
    while let Some((size, tail)) = rest.split_once("\r\n") {
        let size = usize::from_str_radix(size.trim(), 16).unwrap();
        if size == 0 {
            break;
        }
        out.push_str(&tail[..size]);
        rest = &tail[size + 2..];
    }
    out
}

fn request(
    port: u16,
    method: &str,
    path: &str,
    body_json: &str,
    tenant: Option<&str>,
) -> (u16, String) {
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .unwrap();
    let tenant = tenant.map_or(String::new(), |t| format!("x-rushdb-tenant: {t}\r\n"));
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nhost: x\r\nauthorization: Bearer {TOKEN}\r\n{tenant}\
         content-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body_json}",
        body_json.len()
    )
    .unwrap();
    let mut raw = String::new();
    stream.read_to_string(&mut raw).unwrap();
    let status = raw[9..12].parse().unwrap();
    (status, body(&raw))
}

fn execute(port: u16, operation: &Value, tenant: Option<&str>) -> Value {
    let context = tenant.map_or(json!({}), |t| json!({ "scope": t }));
    let request_body = json!({ "protocolVersion": 1, "context": context, "operation": operation });
    let (status, text) = request(
        port,
        "POST",
        "/protocol/v1/execute",
        &request_body.to_string(),
        tenant,
    );
    assert_eq!(status, 200, "{operation}: {text}");
    serde_json::from_str::<Value>(&text).unwrap()["response"].clone()
}

/// Whether `got` has every field of `expected` with the same value.
fn contains(got: &Value, expected: &Value) -> bool {
    match (got, expected) {
        (Value::Object(got), Value::Object(expected)) => expected
            .iter()
            .all(|(key, value)| got.get(key).is_some_and(|g| contains(g, value))),
        (Value::Array(got), Value::Array(expected)) => {
            got.len() == expected.len() && got.iter().zip(expected).all(|(g, e)| contains(g, e))
        }
        _ => got == expected,
    }
}

/// `config.yml` with the listener ports set to 0, everything else as the
/// release wrote it.
fn free_ports(config: &str) -> String {
    config
        .lines()
        .map(|line| match line.split_once(':') {
            Some(("  port" | "  pgwire-port" | "  grpc-port" | "  bolt-port", _)) => {
                format!("{}: 0", line.split(':').next().unwrap())
            }
            _ => line.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn start(
    dir: &Path,
    multitenant: bool,
) -> (u16, impl Fn(), std::thread::JoinHandle<Result<(), String>>) {
    let (mut router, port) = new_router(ServerConfig {
        data_dir: Some(dir.to_path_buf()),
        multitenant,
        token: TOKEN.into(),
        port: 0,
        pgwire_port: 0,
        ..Default::default()
    })
    .unwrap_or_else(|error| panic!("start on {}: {error}", dir.display()));
    let stop = router.shutdown_handle();
    let server = std::thread::spawn(move || router.serve_forever());
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    while request(port, "GET", "/ready", "", None).0 != 200 {
        assert!(
            std::time::Instant::now() < deadline,
            "{} not ready",
            dir.display()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    (port, stop, server)
}

#[test]
fn data_directories_of_every_release_open_and_answer_as_they_did() {
    let workload = read_json(&fixtures().join("workload.json"));
    for release in releases() {
        for mode in MODES {
            let fixture = fixtures().join(&release);
            let expected = read_json(&fixture.join(format!("{mode}.answers.json")));
            let dir = rushdb_storage::TempPath::dir(&format!("upgrade-{release}-{mode}"));
            copy_dir(&fixture.join(mode), &dir);
            let config = std::fs::read_to_string(dir.join("config.yml")).unwrap();
            std::fs::write(dir.join("config.yml"), free_ports(&config)).unwrap();
            let multitenant = mode == "multitenant";
            let tenant = multitenant.then_some(TENANTS[0]);

            let (port, stop, server) = start(&dir, multitenant);
            for (tenant_key, reads) in expected.as_object().unwrap() {
                let scope = (!tenant_key.is_empty()).then_some(tenant_key.as_str());
                for (name, answer) in reads.as_object().unwrap() {
                    let got = execute(port, &workload["reads"][name], scope);
                    assert!(
                        contains(&got, answer),
                        "{release} {mode} {tenant_key} {name}:\nexpected {answer}\ngot {got}"
                    );
                }
            }
            execute(port, &workload["write"], tenant);
            stop();
            server.join().unwrap().expect("clean shutdown");

            let (port, stop, server) = start(&dir, multitenant);
            let employees = execute(port, &workload["reads"]["employees"], tenant);
            let written = employees["records"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|record| record["properties"]["name"] == "Dana")
                .count();
            assert_eq!(written, 1, "{release} {mode}: the write before the restart");
            stop();
            server.join().unwrap().expect("clean shutdown");
            std::fs::remove_dir_all(&*dir).unwrap();
        }
    }
}

/// Every key a release wrote into `config.yml` still loads: a removed one
/// needs a compatibility path (`config::REMOVED_KEYS`), not a startup error.
/// The `vector-exact-max-pool: 5000` every release wrote reads as `auto`.
#[test]
fn every_released_config_file_loads() {
    for release in releases() {
        for mode in MODES {
            let dir = fixtures().join(&release).join(mode);
            let file = rushdb_server::config::ConfigFile::load(&dir)
                .unwrap_or_else(|error| panic!("{release} {mode}: {error}"))
                .expect("a config.yml");
            assert_eq!(file.tenancy.mode, mode, "{release}");
            let resources = file
                .resources
                .unwrap_or_else(|| panic!("{release} {mode}: no resources"));
            assert_eq!(
                resources.vector_exact_max_pool,
                rushdb_server::resource_config::ExactPool::Auto,
                "{release} {mode}"
            );
        }
    }
}

/// The fixtures cover the release before this one, so a release adds its own
/// (see RELEASING.md).
#[test]
fn the_previous_release_has_a_fixture() {
    let version: Vec<u64> = env!("CARGO_PKG_VERSION")
        .split('.')
        .map(|part| part.parse().unwrap())
        .collect();
    let releases = releases();
    assert_eq!(releases.first().map(String::as_str), Some("3.0.0"));
    if version[2] > 0 {
        let previous = format!("{}.{}.{}", version[0], version[1], version[2] - 1);
        assert!(
            releases.contains(&previous),
            "no fixture for {previous}: {releases:?}"
        );
    }
}

#[test]
fn a_misspelt_setting_still_refuses_to_start() {
    let dir = rushdb_storage::TempPath::dir("upgrade-typo");
    let config = std::fs::read_to_string(fixtures().join("3.0.10/single/config.yml")).unwrap();
    let typo = free_ports(&config).replace("  shards: 1", "  shards: 1\n  shard: 1");
    std::fs::write(dir.join("config.yml"), typo).unwrap();
    let error =
        rushdb_server::config::ConfigFile::load(&dir).expect_err("a misspelt setting is refused");
    assert!(
        error.contains("unknown field") && error.contains("shard"),
        "{error}"
    );
    std::fs::remove_dir_all(&*dir).unwrap();
}
