//! `rushdb`: `rushdb serve` runs the database server, `rushdb up` runs it as a
//! persistent local node, and `rushdb health` probes a running node.
mod serve;

use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use std::io::{Read, Write};

#[derive(Debug, Parser)]
#[command(
    name = "rushdb",
    version,
    propagate_version = true,
    about = "RushDB, the LMPG database"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Run the database server. Without `--data-dir` the node is in memory.
    Serve(serve::ServeArgs),
    /// Run a persistent local node: `serve` with a `./.rushdb` data directory
    /// (`./.rushdb/data` when `./.rushdb` holds this binary) and a startup
    /// summary. Takes every `serve` option.
    Up(serve::ServeArgs),
    /// Check that a node is up: exits 0 when `GET /health` answers 200.
    Health {
        /// Base URL of the node (plain HTTP).
        #[arg(long, default_value = "http://127.0.0.1:3000", env = "RUSHDB_URL")]
        url: String,
    },
}

/// The CLI with `up`'s own data-directory default.
fn command() -> clap::Command {
    Cli::command().mut_subcommand("up", |up| {
        up.mut_arg("data_dir", |arg| {
            arg.default_value("./.rushdb")
                .help("Persistent database directory")
        })
    })
}

fn main() {
    let matches = command().get_matches();
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|error| error.exit());
    let explicit_flags = |name: &str| {
        let command = command();
        let subcommand = command
            .find_subcommand(name)
            .expect("the parsed subcommand exists");
        let sub_matches = matches
            .subcommand_matches(name)
            .expect("the parsed subcommand has matches");
        serve::explicit_flags(subcommand, sub_matches)
    };
    match cli.command {
        Commands::Serve(args) => serve::run(args, explicit_flags("serve")),
        Commands::Up(mut args) => {
            let explicit = explicit_flags("up");
            if !explicit.contains("data-dir") {
                let cwd = std::env::current_dir().unwrap_or_else(|_| ".".into());
                let exe = std::env::current_exe().ok();
                match up_data_dir(&cwd, exe.as_deref()) {
                    UpDataDir::Local => {}
                    UpDataDir::BesideBinary(dir) => args.data_dir = Some(dir),
                    UpDataDir::SharedWithBinary(dir) => println!(
                        "Using the existing store in {}, which also holds the installed binary; \
                         to keep them apart, stop the node and move the store into {}",
                        dir.display(),
                        dir.join(UP_DATA_SUBDIR).display()
                    ),
                }
            }
            print_summary(&args);
            serve::run(args, explicit);
        }
        Commands::Health { url } => match health(&url) {
            Ok(body) => println!("{body}"),
            Err(error) => {
                eprintln!("rushdb health: {error}");
                std::process::exit(1);
            }
        },
    }
}

/// Where `rushdb up` keeps its data when `--data-dir` is not given.
#[derive(Debug, PartialEq, Eq)]
enum UpDataDir {
    /// `./.rushdb`.
    Local,
    /// `./.rushdb` holds the running binary (`install.sh` installs into
    /// `~/.rushdb/bin`, so `rushdb up` from the home directory): the data
    /// goes into this subdirectory of it instead, so deleting the data
    /// never deletes the binary.
    BesideBinary(std::path::PathBuf),
    /// As above, but a store from an earlier release is already in
    /// `./.rushdb` itself: it keeps opening there (nothing is moved).
    SharedWithBinary(std::path::PathBuf),
}

/// The subdirectory of `./.rushdb` `up` uses when `./.rushdb` holds the
/// binary.
const UP_DATA_SUBDIR: &str = "data";

/// `up`'s default data directory for a node started in `cwd` by the
/// binary at `exe`.
fn up_data_dir(cwd: &std::path::Path, exe: Option<&std::path::Path>) -> UpDataDir {
    let local = cwd.join(".rushdb");
    let (Ok(root), Some(Ok(exe))) = (local.canonicalize(), exe.map(std::path::Path::canonicalize))
    else {
        return UpDataDir::Local;
    };
    if !exe.starts_with(&root) {
        return UpDataDir::Local;
    }
    let has_store = ["rushdb.redb", rushdb_server::config::CONFIG_FILE_NAME]
        .iter()
        .any(|file| local.join(file).exists());
    if has_store {
        UpDataDir::SharedWithBinary(local)
    } else {
        UpDataDir::BesideBinary(local.join(UP_DATA_SUBDIR))
    }
}

/// What `rushdb up` prints before the node starts.
fn print_summary(args: &serve::ServeArgs) {
    println!("RushDB {}", env!("CARGO_PKG_VERSION"));
    println!("  HTTP     http://{}:{}", args.listen, args.port);
    if args.pgwire_port != 0 {
        println!(
            "  Postgres postgresql://{}:{}",
            args.listen, args.pgwire_port
        );
    }
    if args.bolt_port != 0 {
        println!("  Bolt     bolt://{}:{}", args.listen, args.bolt_port);
    }
    if let Some(data_dir) = &args.data_dir {
        println!("  Data     {}", data_dir.display());
    }
    if args.token == rushdb_server::DEFAULT_TOKEN {
        println!(
            "  Token    {} (local default; set --token or RUSHDB_TOKEN)",
            rushdb_server::DEFAULT_TOKEN
        );
    }
    println!("Press Ctrl+C to stop.");
}

/// `GET {url}/health` over plain HTTP/1.0; the body when the status is 200.
fn health(url: &str) -> Result<String, String> {
    let rest = url.strip_prefix("http://").ok_or_else(|| {
        format!("`{url}` is not a plain http:// URL; probe HTTPS nodes with curl")
    })?;
    let authority = rest.split('/').next().unwrap_or_default();
    let address = if authority.contains(':') {
        authority.to_string()
    } else {
        format!("{authority}:80")
    };
    let timeout = std::time::Duration::from_secs(3);
    let socket = std::net::ToSocketAddrs::to_socket_addrs(&address)
        .map_err(|error| format!("cannot resolve {address}: {error}"))?
        .next()
        .ok_or_else(|| format!("cannot resolve {address}"))?;
    let mut stream = std::net::TcpStream::connect_timeout(&socket, timeout)
        .map_err(|error| format!("cannot connect to {address}: {error}"))?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|error| error.to_string())?;
    write!(
        stream,
        "GET /health HTTP/1.0\r\nHost: {authority}\r\nConnection: close\r\n\r\n"
    )
    .map_err(|error| format!("cannot send the request: {error}"))?;
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|error| format!("cannot read the response: {error}"))?;
    let (head, body) = response.split_once("\r\n\r\n").unwrap_or((&response, ""));
    let status = head.lines().next().unwrap_or_default();
    if status.split_whitespace().nth(1) == Some("200") {
        Ok(body.trim().to_string())
    } else {
        Err(format!("{status} {}", body.trim()))
    }
}

#[cfg(test)]
mod tests {
    use super::{command, up_data_dir, Cli, Commands, UpDataDir};
    use clap::FromArgMatches;

    fn parse(args: &[&str]) -> Commands {
        let matches = command().try_get_matches_from(args).unwrap();
        Cli::from_arg_matches(&matches).unwrap().command
    }

    #[test]
    fn languages_default_to_the_stable_set() {
        let Commands::Serve(args) = parse(&["rushdb", "serve"]) else {
            panic!("expected serve")
        };
        assert!(args.languages.is_empty(), "{:?}", args.languages);
        let Commands::Serve(args) = parse(&["rushdb", "serve", "--languages", "sql,cypher"]) else {
            panic!("expected serve")
        };
        assert_eq!(args.languages, ["sql", "cypher"]);
    }

    /// A directory tree for `up_data_dir`: `cwd` with the given files.
    fn tree(name: &str, files: &[&str]) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "rushdb-up-{name}-{}-{}",
            std::process::id(),
            uuid::Uuid::now_v7()
        ));
        for file in files {
            let path = root.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, b"").unwrap();
        }
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn up_keeps_data_apart_from_a_binary_installed_in_its_default_directory() {
        // `install.sh` installs into ~/.rushdb/bin; `rushdb up` from ~.
        let home = tree("installed", &[".rushdb/bin/rushdb"]);
        let exe = home.join(".rushdb/bin/rushdb");
        assert_eq!(
            up_data_dir(&home, Some(&exe)),
            UpDataDir::BesideBinary(home.join(".rushdb/data"))
        );
        std::fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn up_keeps_opening_a_store_that_already_shares_the_binary_directory() {
        for store_file in [".rushdb/rushdb.redb", ".rushdb/config.yml"] {
            let home = tree("shared", &[".rushdb/bin/rushdb", store_file]);
            let exe = home.join(".rushdb/bin/rushdb");
            assert_eq!(
                up_data_dir(&home, Some(&exe)),
                UpDataDir::SharedWithBinary(home.join(".rushdb")),
                "{store_file}"
            );
            std::fs::remove_dir_all(home).unwrap();
        }
    }

    #[test]
    fn up_uses_the_local_directory_when_the_binary_lives_elsewhere() {
        let project = tree("project", &[".rushdb/config.yml", "bin/rushdb"]);
        // A binary outside ./.rushdb, an existing or a missing ./.rushdb.
        assert_eq!(
            up_data_dir(&project, Some(&project.join("bin/rushdb"))),
            UpDataDir::Local
        );
        let empty = tree("empty", &["bin/rushdb"]);
        assert_eq!(
            up_data_dir(&empty, Some(&empty.join("bin/rushdb"))),
            UpDataDir::Local
        );
        assert_eq!(up_data_dir(&empty, None), UpDataDir::Local);
        std::fs::remove_dir_all(project).unwrap();
        std::fs::remove_dir_all(empty).unwrap();
    }

    #[test]
    fn up_keeps_data_in_a_local_directory_and_serve_in_memory() {
        let Commands::Up(args) = parse(&["rushdb", "up", "--bolt-port", "7687"]) else {
            panic!("expected up")
        };
        assert_eq!(
            args.data_dir.as_deref(),
            Some(std::path::Path::new("./.rushdb"))
        );
        assert_eq!(args.bolt_port, 7687);
        let Commands::Serve(args) = parse(&["rushdb", "serve"]) else {
            panic!("expected serve")
        };
        assert_eq!(args.data_dir, None);
    }
}
