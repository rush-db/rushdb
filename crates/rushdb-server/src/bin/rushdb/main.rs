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
    /// and a startup summary. Takes every `serve` option.
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
        Commands::Up(args) => {
            print_summary(&args);
            serve::run(args, explicit_flags("up"));
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
    use super::{command, Cli, Commands};
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
