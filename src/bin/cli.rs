use std::path::PathBuf;

use cli_table::{format::Justify, Cell, Style, Table};
use fs_extra::dir::{self, CopyOptions};
use sataddress::{api::generate_stats, db::Db};

use sataddress::db::models::{Params, Stats};

use ansi_term::{self, Colour};
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[clap(author, version, about = "Sataddress management CLI tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// interacts with the database
    Db {
        #[command(subcommand)]
        db_command: DbCommands,
    },
    /// gets usage stats data
    Stats {},
}

#[derive(Subcommand, Debug)]
enum DbCommands {
    /// initialize the database with fixture data
    Init {
        /// json file to initalize the database with
        #[arg(short, long, value_name = "FIXTURE.json")]
        path: PathBuf,
    },

    /// dump the database into json
    Dump {
        /// json file to initalize the database with
        #[arg(short, long, value_name = "FILE.json")]
        path: PathBuf,
    },
}

#[tokio::main]
async fn main() {
    banner("Sataddress management CLI");
    let cli = Cli::parse();

    match cli.command {
        Commands::Db { db_command } => match db_command {
            DbCommands::Init { path } => {
                db_init(path);
            }
            DbCommands::Dump { path } => {
                db_dump(path);
            }
        },
        Commands::Stats {} => {
            app_stats();
        }
    }
}

/// Imports data from a json dump into the `sled` database
fn db_init(path: PathBuf) {
    let db = Db::init().unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let data: Vec<Params> = serde_json::from_str(&text).unwrap();
    println!("data is {:?}", data);
    for p in data.iter() {
        match db.insert(&p.name, &p.domain, p).unwrap() {
            Some(_) => {
                println!(
                    "[{}] {}@{}",
                    Colour::Yellow.paint("Updated"),
                    p.name,
                    p.domain
                );
            }
            None => {
                println!("[{}] {}@{}", Colour::Green.paint("Added"), p.name, p.domain);
            }
        }
    }
    println!("[{}] Fixture loaded successfully", Colour::Green.paint("✓"));
}

/// Dumps `sled` database into a json file at provided `path`
fn db_dump(path: PathBuf) {
    let db = Db::init().unwrap();
    let mut data = vec![];
    for r in db.iter() {
        let ivec = r.unwrap();
        let p: Params = rmp_serde::from_slice(&ivec.1).unwrap();
        data.push(p);
    }
    std::fs::write(path, serde_json::to_string_pretty(&data).unwrap()).unwrap();
}

/// Prints basic usage statistics for the application
fn app_stats() {
    // yeah that's highly inefficient but once that
    // becomes a problem we should move to an actual
    // telemetry system anyways
    let db = DbCopy::init();
    let (data, summary) = generate_stats(&db.0).unwrap();
    let mut data: Vec<(&String, &Stats)> = data.iter().collect();
    data.sort_by(|a, b| b.1.cmp(a.1));

    let table = vec![
        vec![
            "Invioices generated".cell(),
            summary["invoices"]
                .as_i64()
                .unwrap()
                .cell()
                .justify(Justify::Right),
        ],
        vec![
            "API calls".cell(),
            summary["calls"]
                .as_i64()
                .unwrap()
                .cell()
                .justify(Justify::Right),
        ],
        vec![
            "API edits".cell(),
            summary["edits"]
                .as_i64()
                .unwrap()
                .cell()
                .justify(Justify::Right),
        ],
    ]
    .table()
    .title(vec![
        "Name".cell().bold(true),
        "Total number".cell().bold(true),
    ])
    .bold(true);

    println!("Summary of app operations");
    println!("{}", table.display().unwrap());
    println!(
        "\nPer user operations, ordered desc, top 10. Total users: {}",
        Colour::Yellow.paint(data.len().to_string())
    );

    let max_elems = 10.min(data.len());
    let mut table = vec![];
    for (username, stats) in data[..max_elems].iter() {
        table.push(vec![
            username.cell(),
            stats.invoices.num.cell().justify(Justify::Right),
            stats.calls.num.cell().justify(Justify::Right),
            stats.edits.num.cell().justify(Justify::Right),
        ]);
    }

    let table = table
        .table()
        .title(vec![
            "User name".cell().bold(true),
            "Invoices generated".cell().bold(true),
            "API calls".cell().bold(true),
            "API edits".cell().bold(true),
        ])
        .bold(true);
    println!("{}", table.display().unwrap());
}

/// Prints out the `cli` tool banner
fn banner(quote: &str) {
    const BTC: &str = r"
        ──▄▄█▀▀▀▀▀█▄▄──
        ▄█▀░░▄░▄░░░░▀█▄
        █░░░▀█▀▀▀▀▄░░░█
        █░░░░█▄▄▄▄▀░░░█
        █░░░░█░░░░█░░░█
        ▀█▄░▀▀█▀█▀░░▄█▀
        ──▀▀█▄▄▄▄▄█▀▀──";
    let text = format!("{:-^34}\n{}\n", quote, Colour::Yellow.paint(BTC));
    println!("{}", text);
}

static DEFAULT_TMP_DB: &str = "db.tmp";
/// Creates a temporary copy (on disk) of the database to interact with
/// as `sled` does not allow to have two readers at the same time and
/// we want to be able to interact via `cli` while the server is running
struct DbCopy(Db);

impl DbCopy {
    /// Returns a `Db` struct that is interacting with a copy
    /// of the sled db.
    fn init() -> Self {
        let mut options = CopyOptions::new();
        options.copy_inside = true;
        dir::copy(sataddress::db::DEFAULT_NAME, DEFAULT_TMP_DB, &options).unwrap();

        Self(Db::from_path(DEFAULT_TMP_DB).unwrap())
    }
}

impl Drop for DbCopy {
    /// Removes the temporary copy of the db in the filesystem
    fn drop(&mut self) {
        dir::remove(DEFAULT_TMP_DB).unwrap();
    }
}
