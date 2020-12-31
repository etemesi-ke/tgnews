#![warn(clippy::pedantic)]
#![allow(
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::needless_pass_by_value,
    clippy::non_ascii_literal
)]
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate rocket;
#[macro_use]
extern crate log;

use clap::{App, Arg};

pub mod alexa;
pub mod categories;
pub mod document;
pub mod languages;
pub mod logger;
pub mod news;
pub mod server;
pub mod slink;
pub mod threads;
pub mod utils;
// My modules
// Load static info

/// The main function is synchronous when running in cli mode
/// But is asynchronous when in server mode
#[rocket::main]
async fn main() {
    // Call it to ensure dbase_conn doesn't try saving data in directories that don't exist
    // Command line arguments
    let app = App::new("tgnews")
        .version("1.0")
        .about("Telegram news aggregator")
        .arg(Arg::new("cpu-threads").short('c').takes_value(true))
        .subcommand(
            App::new("languages").about("<source dir>").arg(
                Arg::new("dir")
                    .takes_value(true)
                    .about("source dir")
                    .required(true),
            ),
        )
        .subcommand(
            App::new("news").about("<source dir>").arg(
                Arg::new("dir")
                    .takes_value(true)
                    .about("source dir")
                    .required(true),
            ),
        )
        .subcommand(
            App::new("categories").about("<source dir>").arg(
                Arg::new("dir")
                    .takes_value(true)
                    .about("source dir")
                    .required(true),
            ),
        )
        .subcommand(
            App::new("threads").about("<source dir>").arg(
                Arg::new("dir")
                    .takes_value(true)
                    .about("source dir")
                    .required(true),
            ),
        )
        .subcommand(
            App::new("server").about("<port>").arg(
                Arg::new("port")
                    .takes_value(true)
                    .about("port")
                    .required(true),
            ),
        );

    // extract the matches
    let matches = app.get_matches();
    let thread = matches.value_of_t("cpu-threads").unwrap_or(16);
    match matches.subcommand_name() {
        Some("languages") => crate::languages::entry(
            matches
                .subcommand_matches("languages")
                .unwrap()
                .value_of("dir")
                .unwrap(),
            thread,
        ),
        Some("news") => crate::news::entry(
            matches
                .subcommand_matches("news")
                .unwrap()
                .value_of("dir")
                .unwrap(),
            thread,
        ),
        Some("categories") => crate::categories::classifier_entry(
            matches
                .subcommand_matches("categories")
                .unwrap()
                .value_of("dir")
                .unwrap(),
            thread,
        ),
        Some("threads") => crate::threads::entry(
            matches
                .subcommand_matches("threads")
                .unwrap()
                .value_of("dir")
                .unwrap(),
            thread,
        ),
        Some("server") => {
            crate::server::mount(
                matches
                    .subcommand_matches("server")
                    .unwrap()
                    .value_of("port")
                    .unwrap()
                    .to_string()
                    .parse::<u16>()
                    .expect("Could not convert port to a u16 instance"),
            )
            .await
        }
        None => println!("Unknown command, run tgnews -h for available commands"),
        Some(x) => println!("Unknown command '{}'", x),
    }
}
// Done 🛩
