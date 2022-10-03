use std::path::PathBuf;

use satadress::db::Db;

use satadress::db::models::Params;

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
            todo!();
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
