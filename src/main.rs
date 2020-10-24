use config;
use std::env;
use web3::types::H160;

async fn list_ecr20_for_account(account_address : H160, etherscan_api_key : String) {
    let url = format!("http://api.etherscan.io/api?module=account&action=tokentx&address={:?}&startblock=0&endblock=999999999&sort=asc&apikey={}", account_address, etherscan_api_key);
    println!("{}", url);

    let body = reqwest::get(&url)
    .await
    .unwrap()
    .text()
    .await
    .unwrap();

    println!("body = {:#?}", body);
}

#[tokio::main]
async fn main() -> web3::Result<()> {
    let mut settings = config::Config::default();
    settings.merge(config::File::with_name("Settings")).unwrap();

    let endpoint = format!("https://mainnet.infura.io/v3/{}", settings.get::<String>("infura").unwrap()).to_owned();
    let transport = web3::transports::Http::new(&endpoint)?;
    let web3 = web3::Web3::new(transport);

    let args: Vec<String> = env::args().collect();
    let address = args[1].parse().unwrap();

    println!("Calling balance...");
    let balance = web3.eth().balance(address, None).await?.low_u64();
    let eth_balance : f64 = balance as f64 / 10_u64.pow(18) as f64;
    println!("Balance of {:?}: {:.5} Ξ", address, eth_balance);

    list_ecr20_for_account(address, settings.get::<String>("etherscan").unwrap()).await;

    Ok(())
}
