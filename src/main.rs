mod lib;

use lib::{coingecko, erc20};

#[macro_use]
extern crate prettytable;
use clap::{App, Arg};
use piechart::{Chart, Color, Data, Style};
use prettytable::Table;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use std::cmp::Ordering::Equal;
use std::error;

fn random_char() -> char {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(1)
        .collect::<Vec<u8>>()[0] as char
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn error::Error>> {
    let app = App::new("portfolio-cli")
        .version("0.1.0")
        .author("Jonathan <JonaC22@users.noreply.github.com>")
        .about("Track balance of ETH and ERC20 tokens easily from cli")
        .arg(
            Arg::with_name("address")
                .short("a")
                .long("address")
                .takes_value(true)
                .required(true)
                .help("ETH address"),
        )
        .arg(
            Arg::with_name("verbose")
                .short("v")
                .long("verbose")
                .takes_value(false)
                .required(false)
                .help("Verbose mode"),
        )
        .get_matches();

    let mut settings = config::Config::default();
    settings.merge(config::File::with_name("Settings"))?;

    let infura_key = settings.get::<String>("infura")?;
    let etherscan_key = settings.get::<String>("etherscan")?;
    let ethplorer_key = settings.get::<String>("ethplorer")?;

    let endpoint = format!("https://mainnet.infura.io/v3/{}", infura_key);
    let transport = web3::transports::Http::new(&endpoint)?;
    let web3 = web3::Web3::new(transport);

    let verbose: bool = app.is_present("verbose");

    let mut raw_address = app.value_of("address").ok_or("No address specified")?;

    if let Some(stripped) = raw_address.strip_prefix("0x") {
        raw_address = stripped;
    }

    let address = raw_address
        .parse::<web3::types::H160>()
        .map_err(|err| format!("Error at specified address: {}. {:?}", raw_address, err))?;

    if verbose {
        println!("Address: {}", address)
    }

    if verbose {
        println!("Calling balance...");
    }
    let balance = web3.eth().balance(address, None).await?.low_u64();
    let eth_balance = balance as f64 / 10_u64.pow(18) as f64;
    let eth_balance_vs_usd =
        eth_balance * coingecko::get_token_price("ethereum", "usd", verbose).await?;

    if verbose {
        println!(
            "ETH balance of {:?}: {:.6} Ξ / {:.2} US$",
            address, eth_balance, eth_balance_vs_usd
        );
    }

    println!("Loading ERC20 token transactions, this will take a while...");

    let list_config = erc20::ListConfig::new(None, None, true, verbose);

    let list_erc20 =
        erc20::list_erc20_for_account(address, &etherscan_key, &ethplorer_key, list_config).await?;

    println!("Balance of ERC20 tokens:");

    let mut total_usd_balance = eth_balance_vs_usd;
    let mut total_eth_balance = eth_balance;

    let mut data = Vec::new();

    let mut rng = rand::thread_rng();
    data.push(Data {
        label: "ETH".into(),
        value: eth_balance_vs_usd as f32,
        color: Some(Style::new().fg(Color::Fixed(rng.gen_range(0..255)))),
        fill: random_char(),
    });

    let mut table = Table::new();
    table.add_row(row![
        "TOKEN",
        "CONTRACT ADDRESS",
        "TOKEN BALANCE",
        "TOTAL ETH",
        "TOTAL USD",
        "COINGECKO LINK"
    ]);

    table.add_row(row![
        "ETH",
        "",
        format!("{:.6}", eth_balance),
        format!("{:.6} Ξ", eth_balance),
        format!("{:.2} US$", eth_balance_vs_usd),
        "https://coingecko.com/en/coins/ethereum".to_string()
    ]);

    for (token_symbol, values) in &list_erc20 {
        match values {
            Some(values) => {
                let balance: f64 = values.balance;
                let usd_balance: f64 = values.usd_balance;
                let eth_balance: f64 = values.eth_balance;
                let coingecko_link: &String = &values.coingecko_link;

                if usd_balance >= 0.01 {
                    total_usd_balance += usd_balance;
                    total_eth_balance += eth_balance;

                    table.add_row(row![
                        token_symbol,
                        values.contract_address,
                        format!("{:.6}", balance),
                        format!("{:.6} Ξ", eth_balance),
                        format!("{:.2} US$", usd_balance),
                        coingecko_link.to_string()
                    ]);

                    let mut rng = rand::thread_rng();
                    data.push(Data {
                        label: token_symbol.into(),
                        value: usd_balance as f32,
                        color: Some(Style::new().fg(Color::Fixed(rng.gen_range(0..255)))),
                        fill: random_char(),
                    });
                }
            }
            None => (),
        }
    }

    table.add_row(row![
        "TOTAL",
        "",
        "",
        format!("{:.6} Ξ", total_eth_balance),
        format!("{:.2} US$", total_usd_balance),
        ""
    ]);

    table.printstd();

    data.sort_by(|a, b| b.value.partial_cmp(&a.value).unwrap_or(Equal));

    Chart::new()
        .radius(20)
        .aspect_ratio(4)
        .legend(true)
        .draw(&data);

    Ok(())
}
