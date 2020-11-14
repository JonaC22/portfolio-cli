mod erc20;

use std::env;

#[tokio::main]
async fn main() -> web3::Result<()> {
    let mut settings = config::Config::default();
    settings.merge(config::File::with_name("Settings")).unwrap();

    let endpoint = format!(
        "https://mainnet.infura.io/v3/{}",
        settings.get::<String>("infura").unwrap()
    );
    let transport = web3::transports::Http::new(&endpoint)?;
    let web3 = web3::Web3::new(transport);

    let args: Vec<String> = env::args().collect();
    let address = args[1].parse().unwrap();

    println!("Calling balance...");
    let balance = web3.eth().balance(address, None).await?.low_u64();
    let eth_balance = balance as f64 / 10_u64.pow(18) as f64;
    let eth_balance_vs_usd = eth_balance * erc20::get_token_price("ethereum", "usd").await;
    println!(
        "Balance of {:?}: {:.5} Ξ / {} US$",
        address, eth_balance, eth_balance_vs_usd
    );

    println!("Loading ERC20 token transactions, this will take a while...");

    let list_erc20 =
        erc20::list_erc20_for_account(address, &settings.get::<String>("etherscan").unwrap()).await;

    println!("Balance of ERC20 tokens:");

    for (token_symbol, values) in &list_erc20 {
        match values {
            Some(values) => {
                let balance: f64 = values.get("balance").unwrap().parse::<f64>().unwrap();
                let usd_balance: f64 =
                    values.get("usd_balance").unwrap().parse::<f64>().unwrap() * balance;
                let eth_balance: f64 =
                    values.get("eth_balance").unwrap().parse::<f64>().unwrap() * balance;

                println!(
                    "{} {} {:.7} {} Ξ / {} US$",
                    token_symbol,
                    values.get("contract_address").unwrap(),
                    balance,
                    eth_balance,
                    usd_balance
                )
            }
            None => (),
        }
    }

    Ok(())
}
