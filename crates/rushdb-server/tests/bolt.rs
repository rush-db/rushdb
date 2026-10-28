//! Bolt (Neo4j wire) transport integration tests. A hand-rolled Bolt client
//! drives a real `BoltServer` over TCP: handshake, HELLO auth, RUN/PULL for
//! read + write Cypher, and BEGIN/COMMIT/ROLLBACK cross-service transactions.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Map, Number, Value};

use rushdb_bolt::value;
use rushdb_bolt::BoltServer;
use rushdb_protocol::EnvelopeHandler;
use rushdb_server::{Node, NodeEnvelopeHandler, ServerConfig};

const MAGIC: [u8; 4] = [0x60, 0x60, 0xB0, 0x17];
const VERSION_5_0: [u8; 4] = [0x00, 0x00, 0x00, 0x05];
const SUCCESS: u8 = 0x70;
const RECORD: u8 = 0x71;
const IGNORED: u8 = 0x7E;
const FAILURE: u8 = 0x7F;

fn boot(token: &str, cypher: bool) -> u16 {
    boot_with(token, cypher, false)
}

fn boot_with(token: &str, cypher: bool, multitenant: bool) -> u16 {
    let mut cfg = ServerConfig {
        port: 0,
        pgwire_port: 0,
        grpc_port: 0,
        bolt_port: 0,
        token: token.to_string(),
        data_dir: None,
        multitenant,
        capability_secret: None,
        raft: None,
        resources: rushdb_server::resource_config::ResourceConfig::default(),
        transactions: rushdb_server::resource_config::TransactionConfig::default(),
        languages: rushdb_protocol::LanguageSet {
            cypher,
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
    let node = Arc::new(Node::bootstrap(&mut cfg).expect("bootstrap"));
    let handler: Arc<dyn EnvelopeHandler> = Arc::new(NodeEnvelopeHandler::new(node));
    let (mut server, port) =
        BoltServer::bind(handler, token.to_string(), "127.0.0.1", 0).expect("bolt bind");
    std::thread::spawn(move || server.serve_forever());
    port
}

struct Client {
    stream: TcpStream,
}

impl Client {
    fn connect(port: u16) -> Client {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .expect("read timeout");
        // Handshake: magic + 4 Bolt 5.0 proposals.
        let mut handshake = Vec::new();
        handshake.extend_from_slice(&MAGIC);
        for _ in 0..4 {
            handshake.extend_from_slice(&VERSION_5_0);
        }
        stream.write_all(&handshake).expect("write handshake");
        let mut version = [0u8; 4];
        stream.read_exact(&mut version).expect("read version");
        assert_eq!(version, VERSION_5_0, "negotiated Bolt 5.0");
        Client { stream }
    }

    #[expect(
        clippy::cast_possible_truncation,
        reason = "the chunk length is bounded by the chunker (<= 65535), so the usize-to-u16 conversion cannot truncate"
    )]
    fn send(&mut self, message: &[u8]) {
        let mut offset = 0;
        while offset < message.len() {
            let chunk = (message.len() - offset).min(65_535);
            self.stream
                .write_all(&(chunk as u16).to_be_bytes())
                .expect("write chunk header");
            self.stream
                .write_all(&message[offset..offset + chunk])
                .expect("write chunk");
            offset += chunk;
        }
        self.stream.write_all(&[0, 0]).expect("write terminator");
    }

    fn recv(&mut self) -> Value {
        let mut payload = Vec::new();
        loop {
            let mut len = [0u8; 2];
            self.stream.read_exact(&mut len).expect("read chunk header");

            let chunk = u16::from_be_bytes(len) as usize;
            if chunk == 0 {
                return value::decode(&payload).expect("decode message");
            }
            let start = payload.len();
            payload.resize(start + chunk, 0);
            self.stream
                .read_exact(&mut payload[start..])
                .expect("read chunk");
        }
    }

    fn call(&mut self, tag: u8, fields: Vec<Value>) -> Value {
        let mut out = Vec::new();
        value::encode_struct(tag, fields, &mut out);
        self.send(&out);
        self.recv()
    }

    fn hello(&mut self, token: &str) -> Value {
        self.hello_db(token, None)
    }

    fn hello_db(&mut self, token: &str, db: Option<&str>) -> Value {
        let mut meta = Map::new();
        if let Some(db) = db {
            meta.insert("db".into(), Value::String(db.into()));
        }
        meta.insert("scheme".into(), Value::String("basic".into()));
        meta.insert("principal".into(), Value::String(token.into()));
        meta.insert("credentials".into(), Value::String(token.into()));
        meta.insert("user_agent".into(), Value::String("rust-bolt-test".into()));
        self.call(0x01, vec![Value::Object(meta)])
    }

    fn run(&mut self, statement: &str) -> Value {
        let params = Map::new();
        self.call(
            0x10,
            vec![Value::String(statement.into()), Value::Object(params)],
        )
    }

    fn pull(&mut self) -> Vec<Value> {
        let mut meta = Map::new();
        meta.insert("n".into(), Value::Number(Number::from(-1)));
        self.send(&encode_msg(0x3F, vec![Value::Object(meta)]));
        let mut messages = Vec::new();
        loop {
            let msg = self.recv();
            let is_success = value::struct_tag(&msg) == Some(SUCCESS);
            let is_failure = value::struct_tag(&msg) == Some(FAILURE);
            messages.push(msg);
            if is_success || is_failure {
                break;
            }
        }
        messages
    }

    fn begin(&mut self) -> Value {
        self.call(0x11, vec![Value::Object(Map::new())])
    }

    fn commit(&mut self) -> Value {
        self.call(0x12, vec![])
    }

    fn rollback(&mut self) -> Value {
        self.call(0x13, vec![])
    }
}

fn encode_msg(tag: u8, fields: Vec<Value>) -> Vec<u8> {
    let mut out = Vec::new();
    value::encode_struct(tag, fields, &mut out);
    out
}

fn tag_of(value: &Value) -> u8 {
    value::struct_tag(value).expect("struct response")
}

#[test]
fn bolt_handshake_hello_and_auth() {
    let port = boot("bolt-token", true);
    let mut client = Client::connect(port);
    let hello = client.hello("bolt-token");
    assert_eq!(tag_of(&hello), SUCCESS, "{hello}");
    let meta = value::struct_fields(&hello)[0].as_object().unwrap().clone();
    assert_eq!(meta["server"], "RushDB/1.0");
}

#[test]
fn bolt_hello_bad_credentials_fails() {
    let port = boot("bolt-token", true);
    let mut client = Client::connect(port);
    let hello = client.hello("wrong");
    assert_eq!(tag_of(&hello), FAILURE);
    let meta = value::struct_fields(&hello)[0].as_object().unwrap().clone();
    assert_eq!(meta["code"], "Neo.ClientError.Security.Unauthorized");
}

#[test]
fn bolt_read_and_write_cypher_round_trip() {
    let port = boot("bolt-token", true);
    let mut client = Client::connect(port);
    assert_eq!(tag_of(&client.hello("bolt-token")), SUCCESS);

    // Write: CREATE via RUN (write path -> Mutation::Statement).
    let run = client.run("CREATE (a:Artist {name: 'ada'}) RETURN a.name AS name");
    assert_eq!(tag_of(&run), SUCCESS, "{run}");
    let fields = value::struct_fields(&run)[0]["fields"]
        .as_array()
        .unwrap()
        .clone();
    assert!(!fields.is_empty());

    let pulled = client.pull();
    let records: Vec<&Value> = pulled.iter().filter(|m| tag_of(m) == RECORD).collect();
    assert_eq!(records.len(), 1, "{pulled:?}");
    assert_eq!(value::struct_fields(records[0])[0][0], "ada");

    // Read: MATCH via read path (Query::Statement).
    let run = client.run("MATCH (a:Artist) RETURN a.name AS name");
    assert_eq!(tag_of(&run), SUCCESS, "{run}");
    let pulled = client.pull();
    let records: Vec<&Value> = pulled.iter().filter(|m| tag_of(m) == RECORD).collect();
    assert_eq!(records.len(), 1, "{pulled:?}");
    assert_eq!(value::struct_fields(records[0])[0][0], "ada");
}

#[test]
fn bolt_transaction_commit_persists_rollback_does_not() {
    let port = boot("bolt-token", true);
    let mut client = Client::connect(port);
    assert_eq!(tag_of(&client.hello("bolt-token")), SUCCESS);

    // BEGIN -> CREATE -> COMMIT: the write persists.
    assert_eq!(tag_of(&client.begin()), SUCCESS);
    assert_eq!(
        tag_of(&client.run("CREATE (p:Person {name: 'commit'})")),
        SUCCESS
    );
    client.pull();
    assert_eq!(tag_of(&client.commit()), SUCCESS);

    let run = client.run("MATCH (p:Person {name: 'commit'}) RETURN p.name AS name");
    assert_eq!(tag_of(&run), SUCCESS);
    let pulled = client.pull();
    assert_eq!(pulled.iter().filter(|m| tag_of(m) == RECORD).count(), 1);

    // BEGIN -> CREATE -> ROLLBACK: the write does not persist.
    assert_eq!(tag_of(&client.begin()), SUCCESS);
    assert_eq!(
        tag_of(&client.run("CREATE (p:Person {name: 'rollback'})")),
        SUCCESS
    );
    client.pull();
    assert_eq!(tag_of(&client.rollback()), SUCCESS);

    let run = client.run("MATCH (p:Person {name: 'rollback'}) RETURN p.name AS name");
    assert_eq!(tag_of(&run), SUCCESS);
    let pulled = client.pull();
    assert_eq!(pulled.iter().filter(|m| tag_of(m) == RECORD).count(), 0);
}

#[test]
fn bolt_read_inside_transaction_sees_committed_data() {
    let port = boot("bolt-token", true);
    let mut client = Client::connect(port);
    assert_eq!(tag_of(&client.hello("bolt-token")), SUCCESS);
    client.run("CREATE (p:Person {name: 'committed'})");
    client.pull();

    // A read in a transaction reads committed data, as SDK reads in a
    // transaction do: it sees neither nothing nor the transaction's own writes.
    let names = |client: &mut Client| -> Vec<Value> {
        let run = client.run("MATCH (p:Person) RETURN p.name AS name ORDER BY name");
        assert_eq!(tag_of(&run), SUCCESS, "{run}");
        client
            .pull()
            .iter()
            .filter(|m| tag_of(m) == RECORD)
            .map(|m| value::struct_fields(m)[0][0].clone())
            .collect()
    };
    assert_eq!(tag_of(&client.begin()), SUCCESS);
    client.run("CREATE (p:Person {name: 'staged'})");
    client.pull();
    assert_eq!(names(&mut client), vec![Value::from("committed")]);
    assert_eq!(tag_of(&client.commit()), SUCCESS);
    assert_eq!(
        names(&mut client),
        vec![Value::from("committed"), Value::from("staged")]
    );
}

#[test]
fn bolt_reset_ends_the_open_transaction() {
    let port = boot("bolt-token", true);
    let mut client = Client::connect(port);
    assert_eq!(tag_of(&client.hello("bolt-token")), SUCCESS);

    assert_eq!(tag_of(&client.begin()), SUCCESS);
    client.run("CREATE (p:Person {name: 'staged'})");
    client.pull();
    let reset = client.call(0x0F, vec![]);
    assert_eq!(tag_of(&reset), SUCCESS, "{reset}");

    // After RESET a RUN auto-commits instead of joining the abandoned transaction.
    assert_eq!(tag_of(&client.commit()), FAILURE);
    client.call(0x0F, vec![]);
    client.run("CREATE (p:Person {name: 'after-reset'})");
    client.pull();
    let run = client.run("MATCH (p:Person) RETURN p.name AS name");
    assert_eq!(tag_of(&run), SUCCESS, "{run}");
    let names: Vec<Value> = client
        .pull()
        .iter()
        .filter(|m| tag_of(m) == RECORD)
        .map(|m| value::struct_fields(m)[0][0].clone())
        .collect();
    assert_eq!(names, vec![Value::from("after-reset")]);
}

#[test]
fn bolt_reset_after_failure() {
    let port = boot("bolt-token", true);
    let mut client = Client::connect(port);
    assert_eq!(tag_of(&client.hello("bolt-token")), SUCCESS);

    // A syntax error yields a FAILURE; RESET returns the connection to good.
    let run = client.run("THIS IS NOT CYPHER");
    assert_eq!(tag_of(&run), FAILURE);
    let reset = client.call(0x0F, vec![]);
    assert_eq!(tag_of(&reset), SUCCESS, "{reset}");

    let run = client.run("MATCH (n) RETURN count(n) AS c");
    assert_eq!(tag_of(&run), SUCCESS, "{run}");
}

#[test]
fn bolt_ignores_messages_after_a_failure_until_reset() {
    let port = boot("bolt-token", true);
    let mut client = Client::connect(port);
    assert_eq!(tag_of(&client.hello("bolt-token")), SUCCESS);

    // Drivers pipeline RUN and PULL; the PULL after a failed RUN is ignored, so
    // the RUN's failure is the one the driver reports.
    assert_eq!(tag_of(&client.run("THIS IS NOT CYPHER")), FAILURE);
    let pull = client.call(0x3F, vec![Value::Object(Map::new())]);
    assert_eq!(tag_of(&pull), IGNORED, "{pull}");
    assert_eq!(tag_of(&client.run("RETURN 1 AS x")), IGNORED);
    assert_eq!(tag_of(&client.call(0x0F, vec![])), SUCCESS);
    assert_eq!(tag_of(&client.run("RETURN 1 AS x")), SUCCESS);
}

#[test]
fn bolt_goodbye_closes_cleanly() {
    let port = boot("bolt-token", true);
    let mut client = Client::connect(port);
    assert_eq!(tag_of(&client.hello("bolt-token")), SUCCESS);
    // GOODBYE: no response; the server closes the connection.
    client.send(&encode_msg(0x02, vec![]));
    // A subsequent read should hit EOF.
    let mut buf = [0u8; 2];
    let result = client.stream.read(&mut buf);
    assert!(result.is_err() || result.unwrap() == 0);
}

#[test]
fn bolt_gated_off_without_cypher() {
    // cypher disabled -> Bolt must not bind even with a bolt-port configured.
    let mut cfg = ServerConfig {
        port: 0,
        pgwire_port: 0,
        grpc_port: 0,
        bolt_port: 7687,
        token: "bolt-token".into(),
        data_dir: None,
        multitenant: false,
        capability_secret: None,
        raft: None,
        resources: rushdb_server::resource_config::ResourceConfig::default(),
        transactions: rushdb_server::resource_config::TransactionConfig::default(),
        // `gql` alone does not serve Bolt: Bolt sends every statement as
        // openCypher.
        languages: rushdb_protocol::LanguageSet {
            gql: true,
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
    let node = Arc::new(Node::bootstrap(&mut cfg).expect("bootstrap"));
    let (server, port) = rushdb_server::bind_bolt(&cfg, node).expect("bind_bolt");
    assert_eq!(port, 0);
    assert!(server.is_disabled());
}

#[test]
fn bolt_hello_db_selects_the_tenant_on_a_multitenant_server() {
    let port = boot_with("bolt-token", true, true);
    let mut acme = Client::connect(port);
    assert_eq!(tag_of(&acme.hello_db("bolt-token", Some("acme"))), SUCCESS);
    assert_eq!(
        tag_of(&acme.run("CREATE (p:Person {name: 'Ada'}) RETURN p.name")),
        SUCCESS,
        "a write under the HELLO tenant succeeds"
    );
    acme.pull();
    assert_eq!(tag_of(&acme.run("MATCH (p:Person) RETURN p.name")), SUCCESS);
    let rows = acme.pull();
    assert_eq!(
        rows.iter().filter(|m| tag_of(m) == RECORD).count(),
        1,
        "the tenant reads its own record: {rows:?}"
    );

    let mut other = Client::connect(port);
    assert_eq!(
        tag_of(&other.hello_db("bolt-token", Some("other"))),
        SUCCESS
    );
    assert_eq!(
        tag_of(&other.run("MATCH (p:Person) RETURN p.name")),
        SUCCESS
    );
    let rows = other.pull();
    assert_eq!(
        rows.iter().filter(|m| tag_of(m) == RECORD).count(),
        0,
        "another tenant sees nothing: {rows:?}"
    );
}

#[test]
fn bolt_hello_db_is_refused_on_a_single_tenant_server() {
    let port = boot("bolt-token", true);
    let mut client = Client::connect(port);
    assert_eq!(
        tag_of(&client.hello_db("bolt-token", Some("acme"))),
        SUCCESS
    );
    assert_eq!(
        tag_of(&client.run("MATCH (p:Person) RETURN p.name")),
        FAILURE,
        "a tenant scope must not open a second keyspace"
    );
}
