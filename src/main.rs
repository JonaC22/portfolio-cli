use std::collections::HashMap;
use config;
use std::env;
use web3::types::H160;
use serde_json as JSON;
use jql;
use serde_json::Value;

type TokenInfo = HashMap<&'static str, String>;
type Tokens = HashMap<String, Option<TokenInfo>>;

fn transform_token_name(raw_token_name : &str) -> String {
    raw_token_name.split(' ').next().unwrap().to_lowercase()
}

async fn get_token_price(raw_token_name : &str, versus_name : &str) -> String {
    let token_name = transform_token_name(raw_token_name);

    let url = format!("https://api.coingecko.com/api/v3/simple/price?ids={}&vs_currencies={}", token_name, versus_name);
    let body = reqwest::get(&url).await.unwrap().text().await.unwrap();
    let json: JSON::Value = serde_json::from_str(&body).unwrap();
    let selector = format!(r#""{}"."{}""#, token_name, versus_name);
    let mix_selector = Some(selector.as_str());

    let results = jql::walker(&json, mix_selector).unwrap_or_default();

    match results {
        Value::Number(value) => {
            value.to_string()
        },
        _ => {
            println!("Error on get token price for {} versus {}", token_name, versus_name);
            "0".to_string()
        }
    }
}

async fn get_erc20_balance_for_account(account_address : H160, etherscan_api_key : &str, contract_address : &str) -> String {
    let url = format!("https://api.etherscan.io/api?module=account&action=tokenbalance&contractaddress={}&address={:?}&tag=latest&apikey={}", contract_address, account_address, etherscan_api_key);
    let body = reqwest::get(&url).await.unwrap().text().await.unwrap();
    let json: JSON::Value = serde_json::from_str(&body).unwrap();
    let mix_selector = Some(r#""result""#);

    let results = jql::walker(&json, mix_selector).unwrap();

    match results {
        Value::String(value) => {
            let balance = value.parse::<f64>().unwrap() / 10_u64.pow(18) as f64;
            balance.to_string()
        },
        _ => panic!("Error on processing ERC20 balance for {}", contract_address)
    }
}

async fn list_erc20_for_account(account_address : H160, etherscan_api_key : &str) -> Tokens {
    let url = format!("http://api.etherscan.io/api?module=account&action=tokentx&address={:?}&startblock=0&endblock=999999999&sort=asc&apikey={}", account_address, etherscan_api_key);
    let body = reqwest::get(&url).await.unwrap().text().await.unwrap();
    let json: JSON::Value = serde_json::from_str(&body).unwrap();
    let mix_selector = Some(r#""result"|{"tokenSymbol", "tokenName", "contractAddress"}"#);

    let results = jql::walker(&json, mix_selector).unwrap();

    match results {
        Value::Array(value) => {
            let mut tokens = Tokens::new();
            for entry in value {
                let token_symbol : String = entry.get("tokenSymbol").unwrap().to_string();

                match tokens.get(&token_symbol) {
                    None => {
                        let mut values : TokenInfo = HashMap::new();
                        let contract_address : &str = entry.get("contractAddress").unwrap().as_str().unwrap();
                        let balance : String = get_erc20_balance_for_account(account_address, etherscan_api_key, contract_address).await;

                        values.insert("contract_address", contract_address.to_string());
                        values.insert("balance", balance);

                        let token_name = entry.get("tokenName").unwrap().as_str().unwrap();

                        let usd_balance = get_token_price(token_name, "usd").await;
                        let eth_balance = get_token_price(token_name, "eth").await;
                        values.insert("usd_balance", usd_balance);
                        values.insert("eth_balance", eth_balance);
                        tokens.insert(token_symbol, Some(values));
                    }
                    _ => continue
                }
            }
            tokens
        },
        _ => panic!("Error on processing the list of ERC20 tokens")
    }
}

#[tokio::main]
async fn main() -> web3::Result<()> {
    let mut settings = config::Config::default();
    settings.merge(config::File::with_name("Settings")).unwrap();

    let endpoint = format!("https://mainnet.infura.io/v3/{}", settings.get::<String>("infura").unwrap());
    let transport = web3::transports::Http::new(&endpoint)?;
    let web3 = web3::Web3::new(transport);

    let args: Vec<String> = env::args().collect();
    let address = args[1].parse().unwrap();

    println!("Calling balance...");
    let balance = web3.eth().balance(address, None).await?.low_u64();
    let eth_balance = balance as f64 / 10_u64.pow(18) as f64;
    let eth_balance_vs_usd = get_token_price("ethereum", "usd").await;
    println!("Balance of {:?}: {:.5} Ξ / {} US$", address, eth_balance, eth_balance_vs_usd);

    println!("Loading ERC20 token transactions, this will take a while...");

    let list_erc20 = list_erc20_for_account(address, &settings.get::<String>("etherscan").unwrap()).await;

    println!("Balance of ERC20 tokens:");

    for (token_symbol, values) in &list_erc20 {
        match values {
            Some(values) => {
                let balance : f64 = values.get("balance").unwrap().parse::<f64>().unwrap();
                let usd_balance : f64 = values.get("usd_balance").unwrap().parse::<f64>().unwrap() * balance;
                let eth_balance : f64 = values.get("eth_balance").unwrap().parse::<f64>().unwrap() * balance;

                println!(
                    "{} {} {:.7} {} Ξ / {} US$",
                    token_symbol,
                    values.get("contract_address").unwrap(),
                    balance,
                    eth_balance,
                    usd_balance
                )
            },
            None => ()
        }
    }

    Ok(())
}
