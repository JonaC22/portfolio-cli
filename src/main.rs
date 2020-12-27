mod erc20;

use clap::{App, Arg};
use piechart::{Chart, Color, Data};
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use std::cmp::Ordering::Equal;

fn random_char() -> char {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(1)
        .collect::<Vec<u8>>()[0] as char
}

#[tokio::main]
async fn main() -> web3::Result<()> {
    let matches = App::new("portfolio-cli")
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
    settings.merge(config::File::with_name("Settings")).unwrap();

    let infura_key = settings
        .get::<String>("infura")
        .unwrap_or_else(|_| panic!("infura key is not set in Settings.toml, exit."));
    let etherscan_key = settings
        .get::<String>("etherscan")
        .unwrap_or_else(|_| panic!("etherscan key is not set in Settings.toml, exit."));

    let endpoint = format!("https://mainnet.infura.io/v3/{}", infura_key);
    let transport = web3::transports::Http::new(&endpoint)?;
    let web3 = web3::Web3::new(transport);

    let verbose: bool = matches.is_present("verbose");

    match matches.value_of("address") {
        None => println!("No address specified, exit."),
        Some(a) => match a.parse::<web3::types::H160>() {
            Ok(address) => {
                if verbose {
                    println!("Address: {}", address)
                }
            }
            Err(_) => println!("Error at specified address: {}", a),
        },
    }

    let address = matches.value_of("address").unwrap().parse().unwrap();

    println!("Calling balance...");
    let balance = web3.eth().balance(address, None).await?.low_u64();
    let eth_balance = balance as f64 / 10_u64.pow(18) as f64;
    let eth_balance_vs_usd = eth_balance * erc20::get_token_price("ethereum", "usd").await;
    println!(
        "ETH balance of {:?}: {:.6} Ξ / {:.2} US$",
        address, eth_balance, eth_balance_vs_usd
    );

    println!("Loading ERC20 token transactions, this will take a while...");

    let list_erc20 = erc20::list_erc20_for_account(address, &etherscan_key).await;

    println!("Balance of ERC20 tokens:");

    let mut total_usd_balance = eth_balance_vs_usd;
    let mut total_eth_balance = eth_balance;

    let mut data = Vec::new();

    let mut rng = rand::thread_rng();
    data.push(Data {
        label: "ETH".into(),
        value: eth_balance_vs_usd as f32,
        color: Some(Color::Fixed(rng.gen_range(0..255))),
        fill: random_char(),
    });

    for (token_symbol, values) in &list_erc20 {
        match values {
            Some(values) => {
                let balance: f64 = values.get("balance").unwrap().parse::<f64>().unwrap();
                let usd_balance: f64 = values.get("usd_balance").unwrap().parse::<f64>().unwrap();
                let eth_balance: f64 = values.get("eth_balance").unwrap().parse::<f64>().unwrap();

                if usd_balance >= 0.01 {
                    total_usd_balance += usd_balance;
                    total_eth_balance += eth_balance;

                    println!(
                        "{} {} {:.6} {:.6} Ξ / {:.2} US$",
                        token_symbol,
                        values.get("contract_address").unwrap(),
                        balance,
                        eth_balance,
                        usd_balance
                    );

                    let mut rng = rand::thread_rng();
                    data.push(Data {
                        label: token_symbol.into(),
                        value: usd_balance as f32,
                        color: Some(Color::Fixed(rng.gen_range(0..255))),
                        fill: random_char(),
                    });
                }
            }
            None => (),
        }
    }

    data.sort_by(|a, b| b.value.partial_cmp(&a.value).unwrap_or(Equal));

    Chart::new()
        .radius(20)
        .aspect_ratio(4)
        .legend(true)
        .draw(&data);

    println!("-----------------------------------------");
    println!(
        "Total balance: {:.6} Ξ / {:.2} US$",
        total_eth_balance, total_usd_balance
    );

    Ok(())
}
