mod lib;

use config::Config;
use lib::{coingecko, erc20, paraswap, price_provider::PriceProvider, random};

#[macro_use]
extern crate prettytable;
use clap::Parser;
use piechart::{Chart, Data, Style};
use prettytable::Table;
use std::cmp::Ordering::Equal;
use std::error;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    address: String,
    #[arg(short, long, default_value_t = false)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn error::Error>> {
    let args = Args::parse();

    let config_builder = config::Config::builder()
        .add_source(config::File::new("Settings.toml", config::FileFormat::Toml));
    let settings = config_builder.build()?;

    let verbose: bool = args.verbose;

    let address = args.address;

    scan_balances(address, settings, verbose).await?;

    Ok(())
}

async fn scan_balances(
    address: String,
    settings: Config,
    verbose: bool,
) -> Result<(), Box<dyn error::Error>> {
    let infura_key = settings.get::<String>("infura")?;
    let etherscan_key = settings.get::<String>("etherscan")?;
    let ethplorer_key = settings.get::<String>("ethplorer")?;

    let endpoint = format!("https://mainnet.infura.io/v3/{}", infura_key);

    let mut raw_address = address;

    if let Some(stripped) = raw_address.strip_prefix("0x") {
        raw_address = stripped.to_owned();
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

    let transport = web3::transports::Http::new(&endpoint)?;
    let web3 = web3::Web3::new(transport);

    let (eth_balance, eth_balance_vs_usd) = get_eth_balance(web3, address, verbose).await?;

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

    let mut data = vec![Data {
        label: "ETH".into(),
        value: eth_balance_vs_usd as f32,
        color: Some(Style::new().fg(random::get_color())),
        fill: random::get_char(),
    }];

    let mut table = Table::new();

    fill_table_with_eth(&mut table, eth_balance, eth_balance_vs_usd);
    fill_table_with_erc20(
        &mut table,
        eth_balance,
        eth_balance_vs_usd,
        list_erc20,
        &mut data,
    );

    table.printstd();

    data.sort_by(|a, b| b.value.partial_cmp(&a.value).unwrap_or(Equal));

    Chart::new()
        .radius(20)
        .aspect_ratio(4)
        .legend(true)
        .draw(&data);

    Ok(())
}

fn fill_table_with_eth(table: &mut Table, eth_balance: f64, eth_balance_vs_usd: f64) {
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
}

fn fill_table_with_erc20(
    table: &mut Table,
    mut total_eth_balance: f64,
    mut total_usd_balance: f64,
    list_erc20: erc20::Tokens,
    data: &mut Vec<Data>,
) {
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

                    data.push(Data {
                        label: token_symbol.into(),
                        value: usd_balance as f32,
                        color: Some(Style::new().fg(random::get_color())),
                        fill: random::get_char(),
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
}

async fn get_eth_balance(
    web3: web3::Web3<web3::transports::Http>,
    address: web3::types::H160,
    verbose: bool,
) -> Result<(f64, f64), Box<dyn error::Error>> {
    let price_provider = coingecko::Coingecko;
    let balance = web3.eth().balance(address, None).await?.low_u64();
    let eth_balance = balance as f64 / 10_u64.pow(18) as f64;
    let eth_balance_vs_usd = eth_balance
        * price_provider
            .get_token_price("ethereum", "usd", verbose)
            .await?;
    Ok((eth_balance, eth_balance_vs_usd))
}

#[cfg(test)]
mod test {
    use super::*;
    use config;
    use web3::types::H160;

    #[tokio::test]
    async fn get_eth_balance_for_account_success() {
        let test_account_address: H160 =
            "000000000000000000000000000000000000dead".parse().unwrap();

        let config_builder = config::Config::builder()
            .add_source(config::File::new("Settings.toml", config::FileFormat::Toml));
        let settings = config_builder.build().unwrap();
        let test_infura_key = settings
            .get::<String>("test_infura")
            .unwrap_or_else(|_| panic!("test infura key is not set in Settings.toml, exit."));

        let endpoint = format!("https://mainnet.infura.io/v3/{}", test_infura_key);
        let transport = web3::transports::Http::new(&endpoint).unwrap();
        let web3 = web3::Web3::new(transport);

        let (eth_balance, eth_balance_vs_usd) = get_eth_balance(web3, test_account_address, false)
            .await
            .unwrap();
        assert_ne!(eth_balance, 0.0);
        assert_ne!(eth_balance_vs_usd, 0.0);
    }

    #[tokio::test]
    async fn fill_table_with_eth_success() {
        let mut table = Table::new();
        let eth_balance = 0.0;
        let eth_balance_vs_usd = 0.0;

        fill_table_with_eth(&mut table, eth_balance, eth_balance_vs_usd);
        assert_eq!(table.len(), 2);
    }

    #[tokio::test]
    async fn fill_table_with_erc20_success() {
        let mut table = Table::new();
        let eth_balance = 0.0;
        let usd_balance = 0.0;

        let test_account_address: H160 =
            "000000000000000000000000000000000000dead".parse().unwrap();
        let config_builder = config::Config::builder()
            .add_source(config::File::new("Settings.toml", config::FileFormat::Toml));
        let settings = config_builder.build().unwrap();
        let test_etherscan_api_key = settings
            .get::<String>("test_etherscan")
            .unwrap_or_else(|_| panic!("test etherscan key is not set in Settings.toml, exit."));
        let test_ethplorer_api_key = settings
            .get::<String>("test_ethplorer")
            .unwrap_or_else(|_| panic!("test ethplorer key is not set in Settings.toml, exit."));

        let list_config = erc20::ListConfig::new(Some(11855520), Some(11855590), false, false);

        let list_erc20 = erc20::list_erc20_for_account(
            test_account_address,
            &test_etherscan_api_key,
            &test_ethplorer_api_key,
            list_config,
        )
        .await
        .unwrap();

        let mut data = vec![Data {
            label: "ETH".into(),
            value: usd_balance as f32,
            color: Some(Style::new().fg(random::get_color())),
            fill: random::get_char(),
        }];

        fill_table_with_erc20(&mut table, eth_balance, usd_balance, list_erc20, &mut data);
        assert_eq!(table.len(), 2);
    }
}
