use crate::config;
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
}
fn sui() -> Command {
    Command::new("config").about("cli program config.")
    .args_conflicts_with_subcommands(true)
    .subcommand_required(true)
    .subcommand(Command::new("get").about("get cli program config."))
    .subcommand(
        Command::new("set").about("set cli program config.")
        .arg(arg!(-p --path <PATH> "Parameter file storage directory.").value_parser(clap::value_parser!(PathBuf)))
        .arg(arg!(-k --keypair <PATH> "Wallet key pair address.").value_parser(clap::value_parser!(PathBuf)))
        .arg(arg!(-r --rpc_url <PATH> "Custom rpc url."))
        .arg(arg!(-w --ws_url <PATH> "Custom websocket url."))
        .arg(arg!(-c --cluster <PATH> "set the cluster.Optional values: Testnet,Mainnet,Devnet,Localnet,Debug."))
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
                        conf.print();
                    }
                    Some(("set", matches)) => {
                        println!("set config");
                    }
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            }
        }
        Some(("aptos", matches)) => match matches.subcommand() {
            Some(("config", matches)) => match matches.subcommand() {
                Some(("get", _)) => {
                    conf.print();
                }
                Some(("set", matches)) => {
                    println!("set config");
                }
                _ => unreachable!(),
            },
            _ => unreachable!(),
        },
        _ => unreachable!(),
    }
    Ok(())
}
