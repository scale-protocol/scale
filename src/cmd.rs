use crate::app::App;
use crate::aptos::config::Config as aptosConfig;
use crate::bot;
use crate::com;
use crate::config::{self, Config};
use crate::sui::{config::Config as suiConfig, tool};
use clap::{arg, Command};
use log::debug;
use std::path::PathBuf;
extern crate chrono;
extern crate env_logger;
extern crate log;

fn cli() -> Command {
    Command::new("Scale contract command line operator.")
        .about("Scale contract command line operator. More https://www.scale.exchange.")
        .version("0.1.0")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .allow_external_subcommands(true)
        .author("scale development team.")
        .arg(arg!(-f --file <CONFIG_FILE> "The custom config file.").value_parser(clap::value_parser!(PathBuf)))
        .arg(arg!(-l --log <LOG> "write log to this file.").value_parser(clap::value_parser!(PathBuf)))
        .subcommand(
            Command::new("sui")
                .about("sui blok chain")
                .subcommand_required(true)
                .arg_required_else_help(true)
                .allow_external_subcommands(true)
                .subcommand(sui())
                .subcommand(sui_trade())
                .subcommand(sui_coin())
                .subcommand(sui_oracle()),
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
                .arg(arg!(-w --write_price_to_db <WRITE_PRICE_TO_DB> "If it is false, price data will not be written to influxdb").default_value("true").value_parser(clap::value_parser!(bool)))
                .arg(arg!(-d --duration <DURATION> "If this option is set, the price of the simple price prediction machine will be updated within the interval. Please set it to a reasonable value in the devnet and testnet to avoid using up coins. Unit is second,e.g. 5").value_parser(clap::value_parser!(u64)))
        )
}

fn sui_coin() -> Command {
    Command::new("coin")
        .about("scale sui coin tool, with devnet and testnet.")
        .args_conflicts_with_subcommands(true)
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("set")
                .about("set subscription ratio.")
                .arg_required_else_help(true)
                .arg(
                    arg!(-r --ratio <RATIO> "How many scales can be exchanged for a sui coin")
                        .value_parser(clap::value_parser!(u64)),
                ),
        )
        .subcommand(
            Command::new("burn")
                .about("Burn scale coin and return sui coin")
                .arg_required_else_help(true)
                .arg(arg!(-c --coin <COIN> "The scale coin to burn")),
        )
        .subcommand(
            Command::new("airdrop")
                .arg_required_else_help(true)
                .about("Airdrop SCALE tokens. In order to prevent malicious operation of robots.")
                .arg(arg!(-c --coin <COIN> "Sui token for payment"))
                .arg(
                    arg!(-a --amount <AMOUNT> "How much scale coin is expected to be redeemed.")
                        .value_parser(clap::value_parser!(u64)),
                ),
        )
}
fn sui_oracle() -> Command {
    Command::new("oracle")
        .about("scale sui oracle tool, with devnet and testnet.")
        .args_conflicts_with_subcommands(true)
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("create_price_feed")
                .arg_required_else_help(true)
                .about("create oracle price feed.")
                .arg(arg!(-s --symbol <SYMBOL> "The symbol of the oracle.")),
        )
        .subcommand(
            Command::new("update_owner")
                .about("update oracle price feed.")
                .arg_required_else_help(true)
                .arg(arg!(-f --feed <FEED> "The price feed address of the oracle."))
                .arg(arg!(-o --owner <OWNER> "The new owner of the oracle.")),
        )
        .subcommand(
            Command::new("update_price")
                .about("update price of oracle.")
                .arg_required_else_help(true)
                .arg(arg!(-f --feed <FEED> "The price feed address of the oracle."))
                .arg(
                    arg!(-p --price <PRICE> "The new price of the oracle.")
                        .value_parser(clap::value_parser!(u64)),
                ),
        )
}
fn sui_trade() -> Command {
    Command::new("trade")
        .about(
            "scale sui trade contract tools , It is used to call the contract program more conveniently.",
        )
        .args_conflicts_with_subcommands(true)
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("create_account")
            .arg_required_else_help(true)
                .about("Create a transaction account.")
                .arg(arg!(-c --coin <COIN> "Used to specify transaction currency.")),
        )
        .subcommand(
            Command::new("deposit")
                .about("Withdrawal of trading account balance.")
                .arg_required_else_help(true)
                .arg(arg!(-t --account <ACCOUNT> "Trading account id."))
                .arg(arg!(-c --coin <COIN> "Coins for deduction"))
                .arg(
                    arg!(-a --amount [AMOUNT] "The amount to deposit. If it is 0, the whole coin will be consumed.")
                        .value_parser(clap::value_parser!(u64)),
                ),
        )
        .subcommand(
            Command::new("withdrawal")
                .about("Cash deposit.")
                .arg_required_else_help(true)
                .arg(arg!(-t --account <ACCOUNT> "Coins for deduction"))
                .arg(
                    arg!(-a --amount <AMOUNT> "The balance to be drawn will fail if the equity is insufficient.")
                        .value_parser(clap::value_parser!(u64)),
                ),
        )
        .subcommand(
            Command::new("add_admin_member")
                .about("Add a member to admin.")
                .arg_required_else_help(true)
                .arg(arg!(-a --admin <ADMIN> "The admin cap object id."))
                .arg(
                    arg!(-m --member <MEMBER> "Member address to be added."),
                ),
        )
        .subcommand(
            Command::new("remove_admin_member")
                .about("Remove a member to admin.")
                .arg_required_else_help(true)
                .arg(arg!(-a --admin <ADMIN> "The admin cap object id."))
                .arg(
                    arg!(-m --member <MEMBER> "Member address to be removed."),
                ),
        )
        .subcommand(
            Command::new("create_market")
                .about("Create a market object.")
                .arg_required_else_help(true)
                .arg(arg!(-c --coin <COIN> "Used to specify transaction currency."))
                .arg(arg!(-s --symbol <SYMBOL> "The transaction pair symbol needs pyth.network to support quotation."))
                .arg(arg!(-p --pyth_id <PYTH_ID> "Pyth.network oracle quote object ID."))
                .arg(arg!(-i --size <SIZE> "The basic unit of open position is 1 by default, and the final position size is equal to size * lot.").value_parser(clap::value_parser!(u64)))
                .arg(arg!(-o --opening_price <OPENING_PRICE> "The opening price of the current day is used to calculate the spread, and the subsequent value will be automatically triggered and updated by the robot.").value_parser(clap::value_parser!(u64)))
                .arg(
                    arg!(-d --description <DESCRIPTION> "The description")
                ),
        )
        .subcommand(
            Command::new("update_max_leverage")
            .arg_required_else_help(true)
                .about("Update the max_leverage of market object.")
                .arg(arg!(-a --admin <ADMIN> "The admin cap object id of market."))
                .arg(arg!(-m --market <MARKET> "The market object id."))
                .arg(arg!(-v --max_leverage <MAX_LEVERAGE> "The maximum leverage of the market will be modified to this value.").value_parser(clap::value_parser!(u8)))
                ,
        )
        .subcommand(
            Command::new("update_margin_fee")
            .arg_required_else_help(true)
                .about("Update the margin_fee of market object.")
                .arg(arg!(-a --admin <ADMIN> "The admin cap object id of market."))
                .arg(arg!(-m --market <MARKET> "The market object id."))
                .arg(arg!(-v --margin_fee <MARGIN_FEE> "The margin fee rate of the market will be modified to this value.").value_parser(clap::value_parser!(f64)))
                ,
        )
        .subcommand(
            Command::new("update_insurance_fee")
            .arg_required_else_help(true)
                .about("Update the insurance_fee of market object.")
                .arg(arg!(-a --admin <ADMIN> "The admin cap object id of market."))
                .arg(arg!(-m --market <MARKET> "The market object id."))
                .arg(arg!(-v --insurance_fee <INSURANCE_FEE> "The insurance fee of the market will be modified to this value.").value_parser(clap::value_parser!(f64)))
                ,
        )
        .subcommand(
            Command::new("update_fund_fee")
            .arg_required_else_help(true)
                .about("Update the fund_fee of market object.")
                .arg(arg!(-a --admin <ADMIN> "The admin cap object id of market."))
                .arg(arg!(-m --market <MARKET> "The market object id."))
                .arg(arg!(-n --manual <MARKET> "Whether it is in manual mode. If the value is not true, the modified value will not be applied to the transaction."))
                .arg(arg!(-v --fund_fee <FUND_FEE> "The fund fee rate of the market will be modified to this value.").value_parser(clap::value_parser!(f64)))
                ,
        )
        .subcommand(
            Command::new("update_spread_fee")
            .arg_required_else_help(true)
                .about("Update the spread_fee of market object.")
                .arg(arg!(-a --admin <ADMIN> "The admin cap object id of market."))
                .arg(arg!(-m --market <MARKET> "The market object id."))
                .arg(arg!(-n --manual <MARKET> "Whether it is in manual mode. If the value is not true, the modified value will not be applied to the transaction."))
                .arg(arg!(-v --spread_fee <SPREAD_FEE> "The spread_fee of the market will be modified to this value.").value_parser(clap::value_parser!(f64)))
                ,
        )
        .subcommand(
            Command::new("update_description")
            .arg_required_else_help(true)
                .about("Update the description of market object.")
                .arg(arg!(-a --admin <ADMIN> "The admin cap object id of market."))
                .arg(arg!(-m --market <MARKET> "The market object id."))
                .arg(arg!(-v --description <DESCRIPTION> "The description of the market will be modified to this value.").value_parser(clap::value_parser!(f64)))
                ,
        )
        .subcommand(
            Command::new("update_status")
                .about("Update the status of market object.")
                .arg_required_else_help(true)
                .arg(arg!(-a --admin <ADMIN> "The admin cap object id of market."))
                .arg(arg!(-m --market <MARKET> "The market object id."))
                .arg(arg!(-v --status <STATUS> r#"The status of the market will be modified to this value.
                1 Normal;
                2. Lock the market, allow closing settlement and not open positions.
                3 The market is frozen, and opening and closing positions are not allowed."#).value_parser(clap::value_parser!(u8)))
                ,
        )
        .subcommand(
            Command::new("update_officer")
            .arg_required_else_help(true)
                .about("Update the officer of market object.This must be run by the contract deployer.")
                .arg(arg!(-m --market <MARKET> "The market object id."))
                .arg(arg!(-v --officer <OFFICER> r#"The officer of the market will be modified to this value.
                1 project team.
                2 Certified Third Party.
                3 Community."#).value_parser(clap::value_parser!(u8)))
                ,
        )
        .subcommand(
            Command::new("add_factory_mould")
            .arg_required_else_help(true)
                .about("Add NFT style.This must be run by the contract deployer.")
                .arg(arg!(-n --name <NAME> "The nft style name."))
                .arg(arg!(-d --description <DESCRIPTION> "The nft description."))
                .arg(arg!(-u --url <URL> "The nft image url."))
                ,
        )
        .subcommand(
            Command::new("remove_factory_mould")
            .arg_required_else_help(true)
                .about("Remove NFT style.This must be run by the contract deployer.")
                .arg(arg!(-n --name <NAME> "The nft style name."))
                ,
        )
        .subcommand(
            Command::new("investment")
            .arg_required_else_help(true)
                .about("Funding the market liquidity pool.")
                .arg(arg!(-m --market <MARKET> "The nft style name."))
                .arg(arg!(-c --coin <COIN> "Coins for deduction"))
                .arg(arg!(-n --name <NAME> "The nft style name. NFT credentials of the specified style will be obtained."))
                ,
        )
        .subcommand(
            Command::new("divestment")
            .arg_required_else_help(true)
                .about("Withdraw funds from the market liquidity pool.")
                .arg(arg!(-m --market <MARKET> "The nft style name."))
                .arg(arg!(-n --nft <NFT> "The NFT certificate."))
                ,
        )
        .subcommand(
            Command::new("generate_upgrade_move_token")
            .arg_required_else_help(true)
                .about("Issue vouchers to nft holders for fund transfer.This must be run by the contract deployer.")
                .arg(arg!(-a --address <ADDRESS> "The wallet address of the user who holds the nft."))
                .arg(arg!(-n --nft <NFT> "The NFT certificate."))
                .arg(arg!(-e --expiration_time <EXPIRATION_TIME> "Voucher validity period."))
                ,
        )
        .subcommand(
            Command::new("divestment_by_upgrade")
            .arg_required_else_help(true)
                .about("The user holding the fund transfer voucher transfers the fund to the new version of the contract.")
                .arg(arg!(-m --market <MARKET> "The market object id."))
                .arg(arg!(-n --nft <NFT> "The NFT certificate."))
                .arg(arg!(-t --move_token <MOVE_TOKEN> "Fund transfer voucher."))
                ,
        )
        .subcommand(
            Command::new("open_position")
            .arg_required_else_help(true)
                .about("Open a position.")
                .arg(arg!(-m --market <MARKET> "The market object id."))
                .arg(arg!(-t --account <ACCOUNT> "The object id for trading account."))
                .arg(arg!(-l --lot <LOT> "The lot.").value_parser(clap::value_parser!(u64)))
                .arg(arg!(-L --leverage <LEVERAGE> "The leverage.").value_parser(clap::value_parser!(u8)))
                .arg(arg!(-p --position_type <POSITION_TYPE> "The position type. 1 full position mode, 2 independent position modes").value_parser(clap::value_parser!(u8)))
                .arg(arg!(-d --direction <DIRECTION> "The direction. 1 buy long, 2 sell short").value_parser(clap::value_parser!(u8)))
                ,
        )
        .subcommand(
            Command::new("close_position")
            .arg_required_else_help(true)
                .about("Close the position.")
                .arg(arg!(-m --market <MARKET> "The market object id."))
                .arg(arg!(-t --account <ACCOUNT> "The object id for trading account."))
                .arg(arg!(-p --position <POSITION> "Position object id."))
                ,
        )
}
fn sui() -> Command {
    Command::new("config")
        .about("cli program config.")
        .args_conflicts_with_subcommands(true)
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(Command::new("get").about("get cli program config."))
        .subcommand(
            Command::new("set")
                .about("set cli program config.")
                .arg_required_else_help(true)
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
    let matches = cli().get_matches();
    let config_file = matches.get_one::<PathBuf>("file");
    let log_file = matches.get_one::<PathBuf>("log");
    init_log(log_file);
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
                Some(("coin", matches)) => com::new_tokio_one_thread().block_on(async {
                    let tool = tool::Tool::new(conf).await?;
                    match matches.subcommand() {
                        Some(("set", matches)) => {
                            tool.coin_set(matches).await?;
                        }
                        Some(("burn", matches)) => {
                            tool.coin_burn(matches).await?;
                        }
                        Some(("airdrop", matches)) => {
                            tool.coin_airdrop(matches).await?;
                        }
                        _ => unreachable!(),
                    }
                    Ok::<(), anyhow::Error>(())
                })?,
                Some(("oracle", matches)) => com::new_tokio_one_thread().block_on(async {
                    let tool = tool::Tool::new(conf).await?;
                    match matches.subcommand() {
                        Some(("create_price_feed", matches)) => {
                            tool.create_price_feed(matches).await?;
                        }
                        Some(("update_owner", matches)) => {
                            tool.update_owner(matches).await?;
                        }
                        Some(("update_price", matches)) => {
                            tool.update_price(matches).await?;
                        }
                        _ => unreachable!(),
                    }
                    Ok::<(), anyhow::Error>(())
                })?,
                Some(("trade", matches)) => com::new_tokio_one_thread().block_on(async {
                    let tool = tool::Tool::new(conf).await?;
                    match matches.subcommand() {
                        Some(("create_account", matches)) => {
                            tool.create_account(matches).await?;
                        }
                        Some(("deposit", matches)) => {
                            tool.deposit(matches).await?;
                        }
                        Some(("add_admin_member", matches)) => {
                            tool.add_admin_member(matches).await?;
                        }
                        Some(("remove_admin_member", matches)) => {
                            tool.remove_admin_member(matches).await?;
                        }
                        Some(("create_market", matches)) => {
                            tool.create_market(matches).await?;
                        }
                        Some(("update_max_leverage", matches)) => {
                            tool.update_max_leverage(matches).await?;
                        }
                        Some(("update_insurance_fee", matches)) => {
                            tool.update_insurance_fee(matches).await?;
                        }
                        Some(("update_margin_fee", matches)) => {
                            tool.update_margin_fee(matches).await?;
                        }
                        Some(("update_fund_fee", matches)) => {
                            tool.update_fund_fee(matches).await?;
                        }
                        Some(("update_status", matches)) => {
                            tool.update_status(matches).await?;
                        }
                        Some(("update_description", matches)) => {
                            tool.update_description(matches).await?;
                        }
                        Some(("update_spread_fee", matches)) => {
                            tool.update_spread_fee(matches).await?;
                        }
                        Some(("update_officer", matches)) => {
                            tool.update_officer(matches).await?;
                        }
                        Some(("add_factory_mould", matches)) => {
                            tool.add_factory_mould(matches).await?;
                        }
                        Some(("remove_factory_mould", matches)) => {
                            tool.remove_factory_mould(matches).await?;
                        }
                        Some(("investment", matches)) => {
                            tool.investment(matches).await?;
                        }
                        Some(("divestment", matches)) => {
                            tool.divestment(matches).await?;
                        }
                        Some(("generate_upgrade_move_token", matches)) => {
                            tool.generate_upgrade_move_token(matches).await?;
                        }
                        Some(("divestment_by_upgrade", matches)) => {
                            tool.divestment_by_upgrade(matches).await?;
                        }
                        Some(("open_position", matches)) => {
                            tool.open_position(matches).await?;
                        }
                        Some(("close_position", matches)) => {
                            tool.close_position(matches).await?;
                        }
                        _ => unreachable!(),
                    }
                    Ok::<(), anyhow::Error>(())
                })?,

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

fn init_log(log_file: Option<&PathBuf>) {
    use chrono::Local;
    use std::io::Write;

    let env = env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "warn");
    let mut l = env_logger::Builder::from_env(env);
    l.format(|buf, record| {
        writeln!(
            buf,
            "{} {} [{}] {}",
            Local::now().format("%Y-%m-%d %H:%M:%S"),
            record.level(),
            record.module_path().unwrap_or("<unnamed>"),
            &record.args()
        )
    });
    if let Some(log_file) = log_file {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_file)
            .unwrap();
        let file = std::io::BufWriter::new(file);
        l.target(env_logger::Target::Pipe(Box::new(file))).init();
    } else {
        l.init();
    }
    debug!("env_logger initialized.");
}
