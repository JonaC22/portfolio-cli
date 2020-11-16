use serde_json::Value;
use std::collections::HashMap;
use web3::types::H160;
use nonzero_ext::*;
use governor::{Quota, RateLimiter};
use std::time::Duration;
use std::thread::sleep;

type TokenInfo = HashMap<&'static str, String>;
type Tokens = HashMap<String, Option<TokenInfo>>;

fn transform_token_name(raw_token_name: &str) -> String {
    raw_token_name.split(' ').next().unwrap().to_lowercase()
}

pub async fn get_token_price(raw_token_name: &str, versus_name: &str) -> f64 {
    let token_name = transform_token_name(raw_token_name);

    let url = format!(
        "https://api.coingecko.com/api/v3/simple/price?ids={}&vs_currencies={}",
        token_name, versus_name
    );
    let body = reqwest::get(&url).await.unwrap().text().await.unwrap();
    let json: Value = serde_json::from_str(&body).unwrap();
    let selector = format!(r#""{}"."{}""#, token_name, versus_name);
    let mix_selector = Some(selector.as_str());

    let results = jql::walker(&json, mix_selector).unwrap_or_default();

    match results {
        Value::Number(value) => value.to_string().parse::<f64>().unwrap(),
        _ => 0.0,
    }
}

pub async fn get_erc20_balance_for_account(
    account_address: H160,
    etherscan_api_key: &str,
    contract_address: &str,
) -> f64 {
    let url = format!("https://api.etherscan.io/api?module=account&action=tokenbalance&contractaddress={}&address={:?}&tag=latest&apikey={}", contract_address, account_address, etherscan_api_key);
    let body = reqwest::get(&url).await.unwrap().text().await.unwrap();
    let json: Value = serde_json::from_str(&body).unwrap();
    let mix_selector = Some(r#""result""#);
    let message_selector = Some(r#""message""#);

    let message = jql::walker(&json, message_selector).unwrap();
    if let Value::String(status) = message {
        if &status != "OK" {
            panic!("Error on processing ERC20 balance for {}", contract_address)
        }
    }

    let results = jql::walker(&json, mix_selector).unwrap();

    match results {
        Value::String(value) => value.parse::<f64>().unwrap() / 10_u64.pow(18) as f64,
        _ => panic!("Error on processing ERC20 balance for {}", contract_address),
    }
}

pub async fn list_erc20_for_account(account_address: H160, etherscan_api_key: &str) -> Tokens {
    let url = format!("http://api.etherscan.io/api?module=account&action=tokentx&address={:?}&startblock=0&endblock=999999999&sort=asc&apikey={}", account_address, etherscan_api_key);
    let body = reqwest::get(&url).await.unwrap().text().await.unwrap();
    let json: Value = serde_json::from_str(&body).unwrap();
    let mix_selector = Some(r#""result"|{"tokenSymbol", "tokenName", "contractAddress"}"#);

    let message_selector = Some(r#""message""#);

    let message = jql::walker(&json, message_selector).unwrap();
    if let Value::String(status) = message {
        if &status != "OK" {
            panic!("Error on processing the list of ERC20 tokens")
        }
    }
    let results = jql::walker(&json, mix_selector).unwrap();

    let limiter = RateLimiter::direct(Quota::per_second(nonzero!(9u32))); // Allow 9 units per second

    match results {
        Value::Array(value) => {
            let mut tokens = Tokens::new();
            for entry in value {
                let token_symbol: String = entry.get("tokenSymbol").unwrap().to_string();

                match tokens.get(&token_symbol) {
                    None => {
                        let mut values: TokenInfo = HashMap::new();
                        let contract_address: &str =
                            entry.get("contractAddress").unwrap().as_str().unwrap();
                        let balance: f64 = get_erc20_balance_for_account(
                            account_address,
                            etherscan_api_key,
                            contract_address,
                        )
                        .await;

                        values.insert("contract_address", contract_address.to_string());
                        values.insert("balance", balance.to_string());

                        let token_name = entry.get("tokenName").unwrap().as_str().unwrap();

                        let token_usd_price = get_token_price(token_name, "usd").await;
                        match limiter.check() {
                            Ok(())=> print!("."),
                            _ => sleep(Duration::from_millis(1000))
                        }
                        let token_eth_price = get_token_price(token_name, "eth").await;
                        match limiter.check() {
                            Ok(())=> print!("."),
                            _ => sleep(Duration::from_millis(1000))
                        }
                        values.insert("usd_price", token_usd_price.to_string());
                        values.insert("eth_price", token_eth_price.to_string());
                        values.insert("usd_balance", (balance * token_usd_price).to_string());
                        values.insert("eth_balance", (balance * token_eth_price).to_string());
                        tokens.insert(token_symbol, Some(values));
                    }
                    _ => continue,
                }
            }
            println!("");
            tokens
        }
        _ => panic!("Error on processing the list of ERC20 tokens"),
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use config;
    use web3::types::H160;

    #[tokio::test]
    async fn get_token_price_success() {
        let price = get_token_price("ethereum", "usd").await;
        assert_ne!(price, 0.0);
    }

    #[tokio::test]
    async fn get_token_price_fail() {
        let price = get_token_price("nonexistingtoken", "usd").await;
        assert_eq!(price, 0.0);
    }

    #[tokio::test]
    async fn get_erc20_balance_for_account_success() {
        let test_account_address: H160 =
            "000000000000000000000000000000000000dead".parse().unwrap();
        let test_contract_address = "0x98b2dE885E916b598f65DeD2fDbb63187EAEf184";
        let mut settings = config::Config::default();
        settings.merge(config::File::with_name("Settings")).unwrap();
        let test_etherscan_api_key = settings.get::<String>("test_etherscan").unwrap();
        let balance = get_erc20_balance_for_account(
            test_account_address,
            &test_etherscan_api_key,
            test_contract_address,
        )
        .await;
        assert_ne!(balance, 0.0);
    }

    #[should_panic(expected = "Error on processing ERC20 balance for 0x98b2dE885E916b598f65DeD2")]
    #[tokio::test]
    async fn get_erc20_balance_for_account_fail() {
        let test_account_address: H160 =
            "000000000000000000000000000000000000dead".parse().unwrap();
        let test_contract_address = "0x98b2dE885E916b598f65DeD2";
        let mut settings = config::Config::default();
        settings.merge(config::File::with_name("Settings")).unwrap();
        let test_etherscan_api_key = settings.get::<String>("test_etherscan").unwrap();
        get_erc20_balance_for_account(
            test_account_address,
            &test_etherscan_api_key,
            test_contract_address,
        )
        .await;
    }
}
