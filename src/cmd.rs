use crate::app::App;
use crate::aptos::config::Config as aptosConfig;
use crate::bot;
use crate::com;
use crate::config::{self, Config};
use crate::sui::{config::Config as suiConfig, tool};
use clap::{arg, ArgAction, Command};
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
        .arg(arg!(-g --gasbudget <GAS_BUDGET> "Gas budget for running module initializers.").value_parser(clap::value_parser!(u64)))
        .subcommand(
            Command::new("sui")
                .about("sui blok chain")
                .subcommand_required(true)
                .arg_required_else_help(true)
                .allow_external_subcommands(true)
                .subcommand(sui())
                .subcommand(sui_trade())
                .subcommand(sui_coin())
                .subcommand(sui_nft())
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
                .arg(arg!(-f --full <FULL> "If set to true, a full node will be started, and it is necessary to specify an external InfluxDB database and SQL database in order to start.").default_value("true").value_parser(clap::value_parser!(bool)))
                .arg(arg!(-d --duration <DURATION> r#"If this option is set, the price of the simple price prediction machine will be updated within the interval.
                 Please set it to a reasonable value in the devnet and testnet to avoid using up coins. Unit is second,e.g. 5.
                  -1 means disable updates, 0 means unlimited time updates."#).value_parser(clap::value_parser!(i64)))
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
                .about("set status.")
                .arg_required_else_help(true)
                .arg(
                    arg!(-s --status <STATUS> "The status of the coin, 1: normal, 2: frozen.")
                        .value_parser(clap::value_parser!(u64)),
                ),
        )
        .subcommand(
            Command::new("burn")
                .about("Burn scale coin and return sui coin")
                .arg_required_else_help(true)
                .arg(arg!(-c --coins <COINS> "The scale coins to burn").action(ArgAction::Append)),
        )
        .subcommand(
            Command::new("airdrop")
                .arg_required_else_help(true)
                .about("Airdrop SCALE tokens. In order to prevent malicious operation of robots.")
                .arg(
                    arg!(-a --amount <AMOUNT> "How much scale coin is expected to be redeemed.")
                        .value_parser(clap::value_parser!(u64)),
                ),
        )
        .subcommand(
            Command::new("mint")
                .arg_required_else_help(true)
                .about("Mint SCALE tokens. In order to prevent malicious operation of robots")
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
            Command::new("update_pyth_price_bat").about("update pyth price bat.")
             .arg(
                arg!(-f --update_fee <UPDATE_FEE> "The budget of the transaction fee.")
                .value_parser(clap::value_parser!(u64)),
            )
            .arg(
            arg!(-i --ids <IDS> "You can find the ids of prices at https://pyth.network/developers/price-feed-ids")
            .action(ArgAction::Append),
            ),
        )
        .subcommand(Command::new("get_latest_vaas").about("get latest vaas."))
        .subcommand(Command::new("update_symbol").about("update symbol map."))
        .subcommand(
            Command::new("get_price")
                .about("get price from china.")
                .arg_required_else_help(true)
                .arg(arg!(-s --symbol <SYMBOL> "The symbol of the oracle.")),
        )
}
fn sui_nft() -> Command {
    Command::new("nft")
        .about("scale sui nft tool, with devnet and testnet.")
        .args_conflicts_with_subcommands(true)
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("mint")
                .arg_required_else_help(true)
                .about("mint a scale nft.")
                .arg(arg!(-n --name <NAME> "The nft style name."))
                .arg(arg!(-d --description <DESCRIPTION> "The nft description."))
                .arg(arg!(-i --img_url <IMG_URL> "The nft ipfs image url.")),
        )
        .subcommand(
            Command::new("mint_multiple")
                .arg_required_else_help(true)
                .about("mint a scale nft.")
                .arg(arg!(-n --name <NAME> "The nft style name."))
                .arg(arg!(-d --description <DESCRIPTION> "The nft description."))
                .arg(arg!(-i --img_url <IMG_URL> "The nft ipfs image url."))
                .arg(
                    arg!(-a --amount <AMOUNT> "The amount of NFT to be obtained.")
                        .value_parser(clap::value_parser!(u64)),
                ),
        )
        .subcommand(
            Command::new("mint_recipient")
                .arg_required_else_help(true)
                .about("mint a scale nft.")
                .arg(arg!(-n --name <NAME> "The nft style name."))
                .arg(arg!(-d --description <DESCRIPTION> "The nft description."))
                .arg(arg!(-i --img_url <IMG_URL> "The nft ipfs image url."))
                .arg(arg!(-r --recipient <RECIPIENT> "The recipient address.")),
        )
        .subcommand(
            Command::new("burn")
                .arg_required_else_help(true)
                .about("burn a scale nft.")
                .arg(arg!(-i --id <id> "The object nft id.")),
        )
        .subcommand(
            Command::new("mint_multiple_recipient")
                .arg_required_else_help(true)
                .about("mint a scale nft.")
                .arg(arg!(-n --name <NAME> "The nft style name."))
                .arg(arg!(-d --description <DESCRIPTION> "The nft description."))
                .arg(arg!(-i --img_url <IMG_URL> "The nft ipfs image url."))
                .arg(
                    arg!(-a --amount <AMOUNT> "The amount of NFT to be obtained.")
                        .value_parser(clap::value_parser!(u64)),
                )
                .arg(arg!(-r --recipient <RECIPIENT> "The recipient address.")),
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
                .about("Create a transaction account."),
        )
        .subcommand(
            Command::new("deposit")
            .about("Cash deposit.")
            .arg_required_else_help(true)
            .arg(arg!(-t --account <ACCOUNT> "Trading account id."))
            .arg(arg!(-c --coins <COINS> "Coins for deduction , If empty, try to automatically obtain").action(ArgAction::Append))
            .arg(
                arg!(-a --amount [AMOUNT] "The amount to deposit. If it is 0, the whole coin will be consumed.")
                .value_parser(clap::value_parser!(u64)),
            ),
        )
        .subcommand(
            Command::new("withdrawal")
            .about("Withdrawal of trading account balance.")
                .arg_required_else_help(true)
                .arg(arg!(-t --account <ACCOUNT> "Trading account id."))
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
            Command::new("create_lsp")
                .about("Create a liquidity pool."),
        )
        .subcommand(
            Command::new("create_market")
                .about("Create a market object.")
                .arg_required_else_help(true)
                .arg(arg!(-s --symbol <SYMBOL> "The transaction pair symbol needs pyth.network to support quotation."))
                .arg(arg!(-i --icon <ICON> "The icon of the market."))
                .arg(arg!(-z --size <SIZE> "The basic unit of open position is 1 by default, and the final position size is equal to size * lot.").value_parser(clap::value_parser!(u64)))
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
            Command::new("trigger_update_opening_price")
            .arg_required_else_help(true)
                .about("trigger update opening price.")
                .arg(arg!(-m --market <MARKET> "The market object id."))
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
                .arg(arg!(-i --issue_time <ISSUE_TIME> "issue time sec.").value_parser(clap::value_parser!(u64)))
                .arg(arg!(-c --coins <COINS> "Coins for deduction,If empty, try to automatically obtain").action(ArgAction::Append))
                .arg(arg!(-n --name <NAME> "The nft style name. NFT credentials of the specified style will be obtained."))
                .arg(
                    arg!(-a --amount <AMOUNT> "The amount of NFT to be obtained.")
                        .value_parser(clap::value_parser!(u64)),
                ),
        )
        .subcommand(
            Command::new("divestment")
            .arg_required_else_help(true)
                .about("Withdraw funds from the market liquidity pool.")
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
            Command::new("open_cross_position")
            .arg_required_else_help(true)
                .about("Open a cross position.")
                .arg(arg!(-s --symbol <SYMBOL> "The market symbol."))
                .arg(arg!(-a --account <ACCOUNT> "The object id for trading account."))
                .arg(arg!(-l --lot <LOT> "The lot.").value_parser(clap::value_parser!(f64)))
                .arg(arg!(-L --leverage <LEVERAGE> "The leverage.").value_parser(clap::value_parser!(u8)))
                .arg(arg!(-d --direction <DIRECTION> "The direction. 1 buy long, 2 sell short").value_parser(clap::value_parser!(u8)))
                .arg(arg!(-o --auto_open_price <AUTO_OPEN_PRICE> "Automatic open price").default_value("0").value_parser(clap::value_parser!(u64)))
                .arg(arg!(-t --stop_surplus_price <STOP_SURPLUS_PRICE> "Automatic profit stop price").default_value("0").value_parser(clap::value_parser!(u64)))
                .arg(arg!(-T --stop_loss_price <STOP_LOSS_PRICE> "Automatic stop loss price").default_value("0").value_parser(clap::value_parser!(u64)))
                ,
        )
        .subcommand(
                Command::new("open_isolated_position")
                .arg_required_else_help(true)
                    .about("Open a isolated position.")
                    .arg(arg!(-s --symbol <SYMBOL> "The market symbol."))
                    .arg(arg!(-a --account <ACCOUNT> "The object id for trading account."))
                    .arg(arg!(-l --lot <LOT> "The lot.").value_parser(clap::value_parser!(f64)))
                    .arg(arg!(-L --leverage <LEVERAGE> "The leverage.").value_parser(clap::value_parser!(u8)))
                    .arg(arg!(-d --direction <DIRECTION> "The direction. 1 buy long, 2 sell short").value_parser(clap::value_parser!(u8)))
                    .arg(arg!(-c --coins <COINS> "Coins for open position.").action(ArgAction::Append))
                    .arg(arg!(-o --auto_open_price <AUTO_OPEN_PRICE> "Automatic open price").default_value("0").value_parser(clap::value_parser!(u64)))
                    .arg(arg!(-t --stop_surplus_price <STOP_SURPLUS_PRICE> "Automatic profit stop price").default_value("0").value_parser(clap::value_parser!(u64)))
                    .arg(arg!(-T --stop_loss_price <STOP_LOSS_PRICE> "Automatic stop loss price").default_value("0").value_parser(clap::value_parser!(u64)))
                    ,
            )
        .subcommand(
            Command::new("close_position")
            .arg_required_else_help(true)
                .about("Close the position.")
                .arg(arg!(-p --position <POSITION> "Position object id."))
                .arg(arg!(-l --lot <LOT> "The lot.").value_parser(clap::value_parser!(f64)))
                .arg(arg!(-t --account <ACCOUNT> "The object id for trading account."))
                ,
        )
        .subcommand(
            Command::new("auto_close_position")
            .arg_required_else_help(true)
                .about("Auto close the position.")
                .arg(arg!(-p --position <POSITION> "Position object id."))
                .arg(arg!(-t --account <ACCOUNT> "The object id for trading account."))
                ,
        )
        .subcommand(
            Command::new("force_liquidation")
            .arg_required_else_help(true)
                .about("Force close the position.")
                .arg(arg!(-p --position <POSITION> "Position object id."))
                .arg(arg!(-t --account <ACCOUNT> "The object id for trading account."))
                ,
        )
        .subcommand(
            Command::new("process_fund_fee")
            .arg_required_else_help(true)
                .about("Process fund fee.")
                .arg(arg!(-t --account <ACCOUNT> "The object id for trading account."))
                ,
        )
        .subcommand(
            Command::new("update_cross_limit_position")
            .arg_required_else_help(true)
                .about("Update cross limit position.")
                .arg(arg!(-p --position <POSITION> "Position object id."))
                .arg(arg!(-l --lot <LOT> "The lot.").value_parser(clap::value_parser!(f64)))
                .arg(arg!(-L --leverage <LEVERAGE> "The leverage.").value_parser(clap::value_parser!(u8)))
                .arg(arg!(-o --auto_open_price <AUTO_OPEN_PRICE> "Automatic open price").default_value("0").value_parser(clap::value_parser!(u64)))
                .arg(arg!(-t --account <ACCOUNT> "The object id for trading account."))
                ,
        )
        .subcommand(
            Command::new("update_isolated_limit_position")
            .arg_required_else_help(true)
                .about("Update cross limit position.")
                .arg(arg!(-p --position <POSITION> "Position object id."))
                .arg(arg!(-l --lot <LOT> "The lot.").value_parser(clap::value_parser!(f64)))
                .arg(arg!(-L --leverage <LEVERAGE> "The leverage.").value_parser(clap::value_parser!(u8)))
                .arg(arg!(-c --coins <COINS> "Coins for open position.").action(ArgAction::Append))
                .arg(arg!(-o --auto_open_price <AUTO_OPEN_PRICE> "Automatic open price").default_value("0").value_parser(clap::value_parser!(u64)))
                .arg(arg!(-t --account <ACCOUNT> "The object id for trading account."))
                ,
        )
        .subcommand(
            Command::new("open_limit_position")
            .arg_required_else_help(true)
            .about("Open limit position.")
            .arg(arg!(-p --position <POSITION> "Position object id."))
            .arg(arg!(-t --account <ACCOUNT> "The object id for trading account."))
            ,
        )
        .subcommand(
            Command::new("update_automatic_price")
            .arg_required_else_help(true)
            .about("Update automatic price.")
            .arg(arg!(-p --position <POSITION> "Position object id."))
            .arg(arg!(-t --account <ACCOUNT> "The object id for trading account."))
            .arg(arg!(-o --auto_open_price <AUTO_OPEN_PRICE> "Automatic open price").default_value("0").value_parser(clap::value_parser!(u64)))
            .arg(arg!(-t --stop_surplus_price <STOP_SURPLUS_PRICE> "Automatic profit stop price").default_value("0").value_parser(clap::value_parser!(u64)))
            .arg(arg!(-T --stop_loss_price <STOP_LOSS_PRICE> "Automatic stop loss price").default_value("0").value_parser(clap::value_parser!(u64)))
            ,
        )
        .subcommand(
            Command::new("isolated_deposit")
            .arg_required_else_help(true)
            .about("Isolated deposit.")
            .arg(arg!(-p --position <POSITION> "Position object id."))
            .arg(arg!(-t --account <ACCOUNT> "The object id for trading account."))
            .arg(arg!(-c --coins <COINS> "Coins for open position.").action(ArgAction::Append))
            .arg(
                arg!(-a --amount [AMOUNT] "The amount to deposit. If it is 0, the whole coin will be consumed.")
                .value_parser(clap::value_parser!(u64)),
            )
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
    let gas_budget = *matches.get_one::<u64>("gasbudget").unwrap_or(&1000);
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
                    let tool = tool::Tool::new(conf, gas_budget).await?;
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
                        Some(("mint", matches)) => {
                            tool.coin_mint(matches).await?;
                        }
                        _ => unreachable!(),
                    }
                    Ok::<(), anyhow::Error>(())
                })?,
                Some(("oracle", matches)) => com::new_tokio_one_thread().block_on(async {
                    let tool = tool::Tool::new(conf, gas_budget).await?;
                    match matches.subcommand() {
                        Some(("create_price_feed", matches)) => {
                            tool.create_price_feed(matches).await?;
                        }
                        Some(("update_pyth_price_bat", matches)) => {
                            tool.update_pyth_price_bat(matches).await?;
                        }
                        Some(("update_all_pyth_price", _matches)) => {
                            tool.update_all_pyth_price().await?;
                        }
                        Some(("get_latest_vaas", matches)) => {
                            tool.get_latest_vaas(matches).await?;
                        }
                        Some(("update_symbol", matches)) => {
                            tool.update_symbol(matches).await?;
                        }
                        Some(("get_price", matches)) => {
                            tool.get_price(matches).await?;
                        }
                        _ => unreachable!(),
                    }
                    Ok::<(), anyhow::Error>(())
                })?,
                Some(("nft", matches)) => com::new_tokio_one_thread().block_on(async {
                    let tool = tool::Tool::new(conf, gas_budget).await?;
                    match matches.subcommand() {
                        Some(("mint", matches)) => {
                            tool.mint(matches).await?;
                        }
                        Some(("mint_multiple", matches)) => {
                            tool.mint_multiple(matches).await?;
                        }
                        Some(("mint_recipient", matches)) => {
                            tool.mint_recipient(matches).await?;
                        }
                        Some(("mint_multiple_recipient", matches)) => {
                            tool.mint_multiple_recipient(matches).await?;
                        }
                        Some(("burn", matches)) => {
                            tool.burn(matches).await?;
                        }
                        _ => unreachable!(),
                    }
                    Ok::<(), anyhow::Error>(())
                })?,
                Some(("trade", matches)) => com::new_tokio_one_thread().block_on(async {
                    let tool = tool::Tool::new(conf, gas_budget).await?;
                    match matches.subcommand() {
                        Some(("create_account", matches)) => {
                            tool.create_account(matches).await?;
                        }
                        Some(("deposit", matches)) => {
                            tool.deposit(matches).await?;
                        }
                        Some(("withdrawal", matches)) => {
                            tool.withdrawal(matches).await?;
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
                        Some(("create_lsp", matches)) => {
                            tool.create_lsp(matches).await?;
                        }
                        Some(("update_max_leverage", matches)) => {
                            tool.update_max_leverage(matches).await?;
                        }
                        Some(("update_insurance_fee", matches)) => {
                            tool.update_insurance_fee(matches).await?;
                        }
                        Some(("trigger_update_opening_price", matches)) => {
                            tool.trigger_update_opening_price(matches).await?;
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
                        Some(("open_cross_position", matches)) => {
                            tool.open_cross_position(matches).await?;
                        }
                        Some(("open_isolated_position", matches)) => {
                            tool.open_isolated_position(matches).await?;
                        }
                        Some(("close_position", matches)) => {
                            tool.close_position(matches).await?;
                        }
                        Some(("auto_close_position", matches)) => {
                            tool.auto_close_position(matches).await?;
                        }
                        Some(("force_liquidation", matches)) => {
                            tool.force_liquidation(matches).await?;
                        }
                        Some(("update_cross_limit_position", matches)) => {
                            tool.update_cross_limit_position(matches).await?;
                        }
                        Some(("update_isolated_limit_position", matches)) => {
                            tool.update_isolated_limit_position(matches).await?;
                        }
                        Some(("open_limit_position", matches)) => {
                            tool.open_limit_position(matches).await?;
                        }
                        Some(("update_automatic_price", matches)) => {
                            tool.update_automatic_price(matches).await?;
                        }
                        Some(("isolated_deposit", matches)) => {
                            tool.isolated_deposit(matches).await?;
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
            bot::app::run(app, config_file, matches, gas_budget)?;
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
        let mut style = buf.style();
        style.set_intense(true);
        writeln!(
            buf,
            "{} {} [{}-{}:{}] {}",
            Local::now().format("%Y-%m-%d %H:%M:%S"),
            record.level(),
            record.module_path().unwrap_or("<unnamed>"),
            record.file().unwrap_or("unknown"),
            record.line().unwrap_or(0),
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
