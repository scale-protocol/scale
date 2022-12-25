use crate::app::App;
use crate::aptos::config::Config as aptosConfig;
use crate::bot;
use crate::config::{self, Config};
use crate::sui::config::Config as suiConfig;
use clap::{arg, Command};
use std::path::PathBuf;
fn cli() -> Command {
    Command::new("Scale contract command line operator.")
        .about("Scale contract command line operator. More https://www.scale.exchange .")
        .version("0.1.0")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .allow_external_subcommands(true)
        .author("scale development team.")
        .arg(arg!(-f --file <CONFIG_FILE> "The custom config file."))
        .subcommand(
            Command::new("sui")
                .about("sui blok chain")
                .subcommand_required(true)
                .arg_required_else_help(true)
                .allow_external_subcommands(true)
                .subcommand(sui()),
        )
        .subcommand(
            Command::new("aptos")
                .about("aptos blok chain")
                .subcommand_required(true)
                .arg_required_else_help(true)
                .allow_external_subcommands(true)
                .subcommand(aptos()),
        )
        .subcommand(
            Command::new("bot")
                .about("Start a settlement robot. Monitor the trading market and close risk positions in a timely manner.")
                .arg(arg!(-T --threads <THREADS> "The number of threads that can be started by the robot, which defaults to the number of system cores.").value_parser(clap::value_parser!(usize)))
                .arg(arg!(-t --tasks <TASKS> "The number of settlement tasks that the robot can open, corresponding to the number of tasks in the tokio, 1 by default.").value_parser(clap::value_parser!(usize)))
                .arg(arg!(-p --port <PORT> "The web server port provides http query service and websocket push service. The default value is 3000. If it is set to 0, the web service is disabled.").value_parser(clap::value_parser!(u64)))
                .arg(arg!(-i --ip <IP> "The IP address bound to the web server. The default is 127.0.0.1."))
                .arg(arg!(-b --blockchain <BLOCKCHAIN> "Target blockchain, optional value: sui , aptos").default_value("sui").value_parser(["sui","aptos"]))
        )
}
fn sui() -> Command {
    Command::new("config")
        .about("cli program config.")
        .args_conflicts_with_subcommands(true)
        .subcommand_required(true)
        .subcommand(Command::new("get").about("get cli program config."))
        .subcommand(
            Command::new("set")
                .about("set cli program config.")
                .arg(
                    arg!(-s --storage <PATH> "Parameter file storage directory.")
                        .value_parser(clap::value_parser!(PathBuf)),
                )
                .arg(
                    arg!(-c --sui_client_config <PATH> "Sui client config file path.")
                        .value_parser(clap::value_parser!(PathBuf)),
                ),
        )
}

fn aptos() -> Command {
    Command::new("config").about("cli program config.")
}

pub fn run() -> anyhow::Result<()> {
    env_logger::init();
    let matches = cli().get_matches();
    let config_file = matches.get_one::<PathBuf>("file");
    match matches.subcommand() {
        Some(("sui", matches)) => {
            let mut conf = suiConfig::default();
            config::config(&mut conf, config_file)?;
            match matches.subcommand() {
                Some(("config", matches)) => match matches.subcommand() {
                    Some(("get", _)) => {
                        (&conf).print();
                    }
                    Some(("set", matches)) => {
                        conf.set_config(matches);
                    }
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            }
        }
        Some(("aptos", matches)) => {
            let mut conf = aptosConfig::default();
            config::config(&mut conf, config_file)?;
            match matches.subcommand() {
                Some(("config", matches)) => match matches.subcommand() {
                    Some(("get", _)) => {
                        (&conf).print();
                    }
                    Some(("set", _matches)) => {
                        println!("set config");
                    }
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            }
        }
        Some(("bot", matches)) => {
            let app: App = matches
                .get_one::<String>("blockchain")
                .unwrap()
                .as_str()
                .into();
            bot::app::run(app, config_file, matches)?;
        }
        _ => unreachable!(),
    }
    Ok(())
}
