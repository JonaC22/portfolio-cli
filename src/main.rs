use config;
use std::env;
use web3::types::H160;
use serde_json as JSON;
use jql;
use serde_json::Value::Array;

async fn list_erc20_for_account(account_address : H160, etherscan_api_key : String) -> Vec<String> {
    let url = format!("http://api.etherscan.io/api?module=account&action=tokentx&address={:?}&startblock=0&endblock=999999999&sort=asc&apikey={}", account_address, etherscan_api_key);
    let body = reqwest::get(&url)
                .await
                .unwrap()
                .text()
                .await
                .unwrap();
    dbg!(&body);
    let json: JSON::Value = serde_json::from_str(&body).unwrap();
    let mix_selector = Some(r#""result"|{"tokenSymbol", "contractAddress"}"#);

    let results = jql::walker(&json, mix_selector).unwrap();

    match results {
        Array(value) => {
            let mut v = value.into_iter().map(|value| value.to_string()).collect::<Vec<_>>();
            v.sort();
            v.dedup();
            v
        },
        _ => panic!("Error on processing the list of ERC20 tokens")
    }
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

    let list_erc20 : Vec<String> = list_erc20_for_account(address, settings.get::<String>("etherscan").unwrap()).await;

    println!("List of ERC20 tokens:");

    for token in list_erc20 {
        println!("{}", token);
    }

    Ok(())
}
