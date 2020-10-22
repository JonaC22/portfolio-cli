use config;
use std::env;

#[tokio::main]
async fn main() -> web3::Result<()> {
    let mut settings = config::Config::default();
    settings.merge(config::File::with_name("Settings")).unwrap();

    let endpoint = format!("https://mainnet.infura.io/v3/{}", settings.get::<String>("key").unwrap()).to_owned();
    let transport = web3::transports::Http::new(&endpoint)?;
    let web3 = web3::Web3::new(transport);

    let args: Vec<String> = env::args().collect();
    let address = args[1].parse().unwrap();

    println!("Calling balance...");
    let balance = web3.eth().balance(address, None).await?.low_u64();
    let eth_balance : f64 = balance as f64 / 10_u64.pow(17) as f64;
    println!("Balance of {:?}: {:.5} Ξ", address, eth_balance);

    Ok(())
}
