use std::path::PathBuf;

use cli_table::format::Justify;
use cli_table::{Cell, Table, Style};
use sataddress::{db::Db, api::generate_stats};

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

fn app_stats() {
    let db = Db::init().unwrap();
    let (data, summary) = generate_stats(db).unwrap();
    let mut data: Vec<(&String, &Stats)> = data.iter().collect();
    data.sort_by(|a, b| b.1.cmp(a.1));

    let table = vec![
        vec!["Invioices generates".cell(), summary["invoices"].as_i64().unwrap().cell().justify(Justify::Right)],
        vec!["API calls".cell(), summary["calls"].as_i64().unwrap().cell().justify(Justify::Right)],
        vec!["API edits".cell(), summary["edits"].as_i64().unwrap().cell().justify(Justify::Right)],
    ]
    .table()
    .title(vec![
        "Name".cell().bold(true),
        "Total number".cell().bold(true),
    ])
    .bold(true);

    println!("Summary of app operations");
    println!("{}", table.display().unwrap());
    println!("\nPer user operations, ordered desc, top 10. Total users: {}", Colour::Yellow.paint(data.len().to_string()));

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

    let table = table.table()
    .title(vec![
        "User name".cell().bold(true),
        "Invoices generated".cell().bold(true),
        "API calls".cell().bold(true),
        "API edits".cell().bold(true),
    ])
    .bold(true);
    println!("{}", table.display().unwrap());
}

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
