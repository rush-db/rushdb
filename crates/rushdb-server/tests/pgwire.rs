//! PostgreSQL wire-protocol adapter end-to-end tests.
use rushdb_protocol::LanguageSet;
use rushdb_server::{new_gateway, ServerConfig};
use std::io::{Read, Write};
use std::net::TcpStream;
use tokio_postgres::{Config, NoTls, SimpleQueryMessage};

fn start(multitenant: bool, capability_secret: Option<&[u8]>) -> u16 {
    start_gateway(multitenant, capability_secret, false).1
}

fn start_gateway(
    multitenant: bool,
    capability_secret: Option<&[u8]>,
    serve_http: bool,
) -> (u16, u16) {
    let config = ServerConfig {
        port: 0,
        pgwire_port: 0,
        grpc_port: 0,
        bolt_port: 0,
        token: "pgwire-token".into(),
        data_dir: None,
        multitenant,
        capability_secret: capability_secret.map(<[u8]>::to_vec),
        raft: None,
        resources: rushdb_server::resource_config::ResourceConfig::default(),
        transactions: rushdb_server::resource_config::TransactionConfig::default(),
        languages: LanguageSet {
            sql: true,
            ..Default::default()
        },
        max_request_bytes: 64 * 1024 * 1024,
        listen_addr: "127.0.0.1".into(),
        tls_cert: None,
        tls_key: None,
        maintenance: false,
        allow_insecure_listen: false,
        require_index_warmup: false,
        request_timeout_ms: 0,
        max_concurrent_requests_per_tenant: 0,
        explicit_flags: std::collections::BTreeSet::new(),
    };
    let (mut http, http_port, mut pgwire, pgwire_port, grpc, grpc_port, bolt, bolt_port) =
        new_gateway(config, 0).expect("gateway bootstrap");
    if serve_http {
        std::thread::spawn(move || http.serve_forever());
    } else {
        drop(http);
    }
    std::thread::spawn(move || pgwire.serve_forever());
    let _ = (grpc, grpc_port, bolt, bolt_port);
    (http_port, pgwire_port)
}

fn start_gateway_timeout(request_timeout_ms: u64) -> (u16, u16) {
    let config = ServerConfig {
        port: 0,
        pgwire_port: 0,
        grpc_port: 0,
        bolt_port: 0,
        token: "pgwire-token".into(),
        data_dir: None,
        multitenant: false,
        capability_secret: None,
        raft: None,
        resources: rushdb_server::resource_config::ResourceConfig::default(),
        transactions: rushdb_server::resource_config::TransactionConfig::default(),
        languages: LanguageSet {
            sql: true,
            ..Default::default()
        },
        max_request_bytes: 64 * 1024 * 1024,
        listen_addr: "127.0.0.1".into(),
        tls_cert: None,
        tls_key: None,
        maintenance: false,
        allow_insecure_listen: false,
        require_index_warmup: false,
        request_timeout_ms,
        max_concurrent_requests_per_tenant: 0,
        explicit_flags: std::collections::BTreeSet::new(),
    };
    let (mut router, http_port, mut pg, pg_port, _grpc, _, _bolt, _) =
        rushdb_server::new_gateway(config, 0).expect("gateway");
    std::thread::spawn(move || router.serve_forever());
    std::thread::spawn(move || pg.serve_forever());
    std::thread::sleep(std::time::Duration::from_millis(60));
    (http_port, pg_port)
}

fn put_record(http_port: u16, scope: Option<&str>, label: &str, properties: &serde_json::Value) {
    let body = serde_json::json!({
        "protocolVersion": 1,
        "context": { "scope": scope },
        "operation": {
            "kind": "mutate",
            "mutation": {
                "kind": "putRecord",
                "mode": "create",
                "label": label,
                "properties": properties,
                "options": {}
            }
        }
    })
    .to_string();
    let mut stream = TcpStream::connect(("127.0.0.1", http_port)).unwrap();
    let scope_header = scope
        .map(|scope| format!("X-RushDB-Tenant: {scope}\r\n"))
        .unwrap_or_default();
    let request = format!(
        "POST /protocol/v1/execute HTTP/1.1\r\nHost: localhost\r\nX-RushDB-Token: pgwire-token\r\nContent-Type: application/json\r\n{scope_header}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
}

async fn connect(
    port: u16,
    database: &str,
    password: &str,
) -> Result<tokio_postgres::Client, tokio_postgres::Error> {
    let mut config = Config::new();
    config
        .host("127.0.0.1")
        .port(port)
        .user("rushdb")
        .dbname(database)
        .password(password);
    let (client, connection) = config.connect(NoTls).await?;
    tokio::spawn(async move {
        let _ = connection.await;
    });
    Ok(client)
}

#[tokio::test]
async fn standard_client_runs_simple_and_extended_queries() {
    let (http_port, port) = start_gateway(false, None, true);
    put_record(
        http_port,
        None,
        "NOTE",
        &serde_json::json!({ "name": "alpha", "position": 7, "active": true }),
    );
    let client = connect(port, "rushdb", "pgwire-token").await.unwrap();

    let messages = client.simple_query("SELECT 1;").await.unwrap();
    let row = messages
        .iter()
        .find_map(|message| match message {
            SimpleQueryMessage::Row(row) => Some(row),
            _ => None,
        })
        .unwrap();
    assert_eq!(row.get(0), Some("1"));

    // Columns come back in select-list order, not sorted by name.
    let messages = client
        .simple_query("SELECT position, name, active FROM NOTE")
        .await
        .unwrap();
    let row = messages
        .iter()
        .find_map(|message| match message {
            SimpleQueryMessage::Row(row) => Some(row),
            _ => None,
        })
        .unwrap();
    let names: Vec<&str> = row
        .columns()
        .iter()
        .map(tokio_postgres::SimpleColumn::name)
        .collect();
    assert_eq!(names, ["position", "name", "active"]);
    assert_eq!(row.get(1), Some("alpha"));

    let rows = client.query("SELECT version()", &[]).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].get::<_, String>(0).starts_with("RushDB pgwire"));

    let rows = client.query("SELECT name FROM NOTE", &[]).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<_, String>("name"), "alpha");

    let rows = client
        .query(
            "SELECT name FROM NOTE WHERE position > $1 AND active = $2",
            &[&6_i64, &true],
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<_, String>("name"), "alpha");

    let statement = client
        .prepare("SELECT position, active FROM NOTE WHERE position > $1")
        .await
        .unwrap();
    assert_eq!(statement.params(), &[tokio_postgres::types::Type::INT8]);
    assert_eq!(statement.columns().len(), 2);
    let rows = client.query(&statement, &[&100_i64]).await.unwrap();
    assert!(rows.is_empty());

    let rows = client.query(&statement, &[&6_i64]).await.unwrap();
    assert_eq!(rows[0].get::<_, i64>("position"), 7);
    assert!(rows[0].get::<_, bool>("active"));
}

#[tokio::test]
async fn standard_client_runs_atomic_sql_mutations() {
    let (http_port, port) = start_gateway(false, None, true);
    // Labels are schema-bearing record tables. Seed the label through the
    // canonical API, then exercise DML wholly through a standard pgwire client.
    put_record(
        http_port,
        None,
        "NOTE",
        &serde_json::json!({ "name": "seed", "position": 0 }),
    );
    let client = connect(port, "rushdb", "pgwire-token").await.unwrap();

    // `execute` uses the extended Parse/Bind/Execute flow. The mutation is
    // committed by the same canonical SQL operation as native Protocol v1 and
    // comes back as a PostgreSQL CommandComplete tag with an affected count.
    assert_eq!(
        client
            .execute(
                "INSERT INTO NOTE (name, position) VALUES ($1, $2)",
                &[&"through-pgwire", &7_i64],
            )
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        client
            .execute(
                "UPDATE NOTE SET position = $1 WHERE name = $2",
                &[&8_i64, &"through-pgwire"],
            )
            .await
            .unwrap(),
        1
    );
    let rows = client
        .query(
            "SELECT position FROM NOTE WHERE name = $1",
            &[&"through-pgwire"],
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<_, i64>("position"), 8);
    assert_eq!(
        client
            .execute("DELETE FROM NOTE WHERE name = $1", &[&"through-pgwire"])
            .await
            .unwrap(),
        1
    );

    // Index DDL is a mutation too, over both the simple and extended flows.
    let created = client
        .simple_query("CREATE INDEX note_position ON NOTE(position)")
        .await
        .unwrap();
    assert!(
        created
            .iter()
            .any(|message| matches!(message, SimpleQueryMessage::CommandComplete(_))),
        "{created:?}"
    );
    client
        .execute("DROP INDEX note_position", &[])
        .await
        .unwrap();
}

#[tokio::test]
async fn database_startup_parameter_isolates_tenants() {
    let (http_port, port) = start_gateway(true, None, true);
    put_record(
        http_port,
        Some("tenant-a"),
        "NOTE",
        &serde_json::json!({ "name": "alpha" }),
    );
    put_record(
        http_port,
        Some("tenant-b"),
        "NOTE",
        &serde_json::json!({ "name": "beta" }),
    );

    let tenant_a = connect(port, "tenant-a", "pgwire-token").await.unwrap();
    let tenant_b = connect(port, "tenant-b", "pgwire-token").await.unwrap();
    let a_rows = tenant_a.query("SELECT name FROM NOTE", &[]).await.unwrap();
    let b_rows = tenant_b.query("SELECT name FROM NOTE", &[]).await.unwrap();

    assert_eq!(a_rows.len(), 1);
    assert_eq!(b_rows.len(), 1);
    assert_eq!(a_rows[0].get::<_, String>("name"), "alpha");
    assert_eq!(b_rows[0].get::<_, String>("name"), "beta");
}

#[tokio::test]
async fn password_authentication_fails_closed() {
    let port = start(false, None);
    let error = connect(port, "rushdb", "wrong-token").await.unwrap_err();
    assert_eq!(
        error.as_db_error().map(|error| error.code().code()),
        Some("28P01")
    );
}

#[tokio::test]
async fn capability_password_is_not_accepted_by_stock_pgwire_auth() {
    let secret = b"pgwire-capability-secret";
    let port = start(true, Some(secret));
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let token = rushdb_server::capability::issue_for_tenant(
        secret,
        "reader",
        "read",
        Some("tenant-a"),
        60,
        now,
    );

    let accepted = connect(port, "tenant-a", &token).await;
    assert!(accepted.is_err());

    let rejected = connect(port, "tenant-b", &token).await.unwrap_err();
    assert_eq!(
        rejected.as_db_error().map(|error| error.code().code()),
        Some("28P01")
    );
}

#[tokio::test]
async fn pgwire_query_with_deadline_config_still_succeeds_fast_queries() {
    // The pgwire path wires `request_timeout_ms` into the executor (the same
    // deadline cancellation the HTTP path uses, which is proven at the executor
    // level). A tight-but-nonzero deadline must not break normal fast queries —
    // deterministic, not a timing flake.
    let (http_port, pg_port) = start_gateway_timeout(500);
    put_record(http_port, None, "N", &serde_json::json!({ "v": 7 }));
    let client = connect(pg_port, "rushdb", "pgwire-token").await.unwrap();
    let rows = client.query("SELECT v FROM N", &[]).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<_, i64>("v"), 7);
}

#[tokio::test]
async fn transaction_control_is_refused_as_unsupported() {
    let port = start(false, None);
    let client = connect(port, "rushdb", "pgwire-token").await.unwrap();
    for statement in [
        "BEGIN",
        "START TRANSACTION",
        "COMMIT",
        "ROLLBACK",
        "SAVEPOINT a",
    ] {
        let simple = client.simple_query(statement).await.unwrap_err();
        let extended = client.query(statement, &[]).await.unwrap_err();
        for error in [simple, extended] {
            let error = error.as_db_error().expect("a database error");
            assert_eq!(error.code().code(), "0A000", "{statement}: {error}");
            assert!(error.message().contains("autocommit"), "{error}");
        }
    }
}

#[tokio::test]
async fn session_settings_bound_the_sessions_own_statements() {
    // QRY-3: a filter without an index walks the label unless the session
    // refuses it, as CockroachDB's `disallow_full_table_scans` does.
    let (http_port, port) = start_gateway(false, None, true);
    for v in 0..3 {
        put_record(http_port, None, "N", &serde_json::json!({ "v": v }));
    }
    let client = connect(port, "rushdb", "pgwire-token").await.unwrap();
    let count = "SELECT count(*) AS c FROM n WHERE v = 1";
    assert_eq!(client.query(count, &[]).await.unwrap().len(), 1);

    client
        .simple_query("SET disallow_full_table_scans = on")
        .await
        .unwrap();
    for error in [
        client.simple_query(count).await.unwrap_err(),
        client.query(count, &[]).await.unwrap_err(),
    ] {
        let error = error.as_db_error().expect("a database error");
        assert_eq!(error.code().code(), "42601", "{error}");
        assert!(
            error.message().contains("declare an ordered index"),
            "{error}"
        );
    }
    // A label within `large_full_scan_rows` still walks.
    client
        .query("SET large_full_scan_rows = 3", &[])
        .await
        .unwrap();
    assert_eq!(client.query(count, &[]).await.unwrap().len(), 1);

    client
        .simple_query("SET statement_timeout = '30s'")
        .await
        .unwrap();
    let shown = client.query("SHOW statement_timeout", &[]).await.unwrap();
    assert_eq!(shown[0].get::<_, String>("statement_timeout"), "30000ms");
    let error = client
        .simple_query("SET work_mem = 'plenty'")
        .await
        .unwrap_err();
    assert_eq!(error.as_db_error().unwrap().code().code(), "22023");

    client.simple_query("RESET ALL").await.unwrap();
    assert_eq!(client.query(count, &[]).await.unwrap().len(), 1);
    let messages = client
        .simple_query("SHOW disallow_full_table_scans")
        .await
        .unwrap();
    let SimpleQueryMessage::Row(row) = &messages[1] else {
        panic!("expected a row, got {messages:?}");
    };
    assert_eq!(row.get("disallow_full_table_scans"), Some("off"));
}
