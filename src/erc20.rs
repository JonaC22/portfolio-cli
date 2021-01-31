use governor::{Quota, RateLimiter};
use indicatif::ProgressBar;
use nonzero_ext::*;
use serde_json::{Error, Value};
use std::collections::HashMap;
use std::convert::TryInto;
use std::io::{self, Write};
use std::thread::sleep;
use std::time::Duration;
use web3::types::H160;

#[derive(Debug)]
pub struct TokenInfo {
    pub contract_address: String,
    pub balance: f64,
    pub usd_price: f64,
    pub eth_price: f64,
    pub usd_balance: f64,
    pub eth_balance: f64,
    pub coingecko_link: String,
}

impl<'a> TokenInfo {
    fn new(
        contract_address: &'a str,
        balance: &'a f64,
        usd_price: &'a f64,
        eth_price: &'a f64,
        coingecko_link: &'a str,
    ) -> TokenInfo {
        TokenInfo {
            contract_address: contract_address.to_string(),
            balance: *balance,
            usd_price: *usd_price,
            eth_price: *eth_price,
            usd_balance: balance * usd_price,
            eth_balance: balance * eth_price,
            coingecko_link: coingecko_link.to_string(),
        }
    }
}

type Tokens = HashMap<String, Option<TokenInfo>>;

async fn get_coingecko_token_id_from_contract_address(
    contract_address: &str,
    verbose: bool,
) -> String {
    let url = format!(
        "https://api.coingecko.com/api/v3/coins/ethereum/contract/{}",
        contract_address
    );
    let mut retry: u32 = 0;
    let max_retries: u32 = 5;

    loop {
        let body = reqwest::get(&url).await.unwrap().text().await.unwrap();
        let result: Result<Value, Error> = serde_json::from_str(&body);

        match result {
            Ok(json) => {
                let mix_selector = Some(r#""id""#);

                let results = jql::walker(&json, mix_selector).unwrap_or_default();

                match results {
                    Value::String(value) => return value.parse::<String>().unwrap(),
                    _ => return "".to_string(),
                }
            }
            _ => {
                if retry > max_retries {
                    panic!("Could not fetch from coingecko: response body: {:?}", &body);
                } else {
                    retry += 1;
                    if verbose {
                        println!(
                            "Failed to fetch from coingecko, retry up to {}, retry number: {}",
                            max_retries, retry
                        );
                    }
                    sleep(Duration::from_millis((2_u32.pow(retry) * 1000).into()));
                }
            }
        }
    }
}

pub async fn get_token_price(token_id: &str, versus_name: &str, verbose: bool) -> f64 {
    let url = format!(
        "https://api.coingecko.com/api/v3/simple/price?ids={}&vs_currencies={}",
        token_id, versus_name
    );
    let mut retry: u32 = 0;
    let max_retries: u32 = 5;

    loop {
        let body = reqwest::get(&url).await.unwrap().text().await.unwrap();
        let result: Result<Value, Error> = serde_json::from_str(&body);
        match result {
            Ok(json) => {
                let selector = format!(r#""{}"."{}""#, token_id, versus_name);
                let mix_selector = Some(selector.as_str());

                let results = jql::walker(&json, mix_selector).unwrap_or_default();

                match results {
                    Value::Number(value) => return value.to_string().parse::<f64>().unwrap(),
                    _ => return 0.0,
                }
            }
            _ => {
                if retry > max_retries {
                    panic!("Could not fetch from coingecko: response body: {:?}", &body);
                } else {
                    retry += 1;
                    if verbose {
                        println!(
                            "Failed to fetch from coingecko, retry up to {}, retry number: {}",
                            max_retries, retry
                        );
                    }
                    sleep(Duration::from_millis((2_u32.pow(retry) * 1000).into()));
                }
            }
        }
    }
}

pub async fn get_token_decimal(ethplorer_api_key: &str, contract_address: &str) -> u32 {
    let url = format!(
        "https://api.ethplorer.io/getTokenInfo/{}?apiKey={}
    ",
        contract_address, ethplorer_api_key
    );
    let body = reqwest::get(&url).await.unwrap().text().await.unwrap();
    let json: Value = serde_json::from_str(&body).unwrap();
    let mix_selector = Some(r#""decimals""#);

    let results = jql::walker(&json, mix_selector).unwrap_or_else(|_| {
        panic!(
            "Error on fetching decimals for token contract {}",
            contract_address
        )
    });

    match results {
        Value::String(value) => value.parse::<u32>().unwrap(),
        Value::Number(value) => value.to_string().parse::<u32>().unwrap(),
        _ => panic!(
            "Error on fetching decimals for token contract {}",
            contract_address
        ),
    }
}

pub async fn get_erc20_balance_for_account(
    account_address: H160,
    etherscan_api_key: &str,
    ethplorer_api_key: &str,
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

    let decimal = get_token_decimal(ethplorer_api_key, contract_address).await;

    match results {
        Value::String(value) => value.parse::<f64>().unwrap() / 10_u64.pow(decimal) as f64,
        _ => panic!("Error on processing ERC20 balance for {}", contract_address),
    }
}

pub async fn list_erc20_for_account(
    account_address: H160,
    etherscan_api_key: &str,
    ethplorer_api_key: &str,
    verbose: bool,
) -> Tokens {
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

    let limiter = RateLimiter::direct(Quota::per_second(nonzero!(8u32))); // Allow 8 units per second

    match results {
        Value::Array(value) => {
            let mut tokens = Tokens::new();
            let pb = ProgressBar::new(value.len().try_into().unwrap());

            for entry in value {
                pb.inc(1);
                io::stdout().flush().unwrap();
                let token_symbol: String = entry.get("tokenSymbol").unwrap().to_string();

                match tokens.get(&token_symbol) {
                    None => {
                        let contract_address: &str =
                            entry.get("contractAddress").unwrap().as_str().unwrap();

                        let token_id: String =
                            get_coingecko_token_id_from_contract_address(contract_address, verbose)
                                .await;

                        let balance: f64 = get_erc20_balance_for_account(
                            account_address,
                            etherscan_api_key,
                            ethplorer_api_key,
                            contract_address,
                        )
                        .await;

                        let token_usd_price_future = get_token_price(&token_id, "usd", verbose);
                        match limiter.check() {
                            Ok(()) => (),
                            _ => sleep(Duration::from_millis(2000)),
                        }

                        let token_eth_price_future = get_token_price(&token_id, "eth", verbose);
                        match limiter.check() {
                            Ok(()) => (),
                            _ => sleep(Duration::from_millis(2000)),
                        }

                        let usd_price = token_usd_price_future.await;
                        let eth_price = token_eth_price_future.await;

                        let token_info: TokenInfo = TokenInfo::new(
                            contract_address,
                            &balance,
                            &usd_price,
                            &eth_price,
                            &format!("https://coingecko.com/en/coins/{}", token_id),
                        );

                        tokens.insert(token_symbol, Some(token_info));
                    }
                    _ => continue,
                }
            }
            pb.finish_with_message("done");
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
    async fn get_token_id_success() {
        // YFI token address
        let erc20_contract_address = "0x0bc529c00C6401aEF6D220BE8C6Ea1667F6Ad93e";
        let id = get_coingecko_token_id_from_contract_address(erc20_contract_address, true).await;
        assert_eq!(id, "yearn-finance");
    }

    #[tokio::test]
    async fn get_token_id_fail() {
        // non existent token address
        let erc20_contract_address = "0x0121212121212121212121212212121212121212";
        let id = get_coingecko_token_id_from_contract_address(erc20_contract_address, true).await;
        assert_eq!(id, "");
    }

    #[tokio::test]
    async fn get_token_decimal_success() {
        // YFI token address
        let erc20_contract_address = "0x0bc529c00C6401aEF6D220BE8C6Ea1667F6Ad93e";
        let mut settings = config::Config::default();
        settings.merge(config::File::with_name("Settings")).unwrap();
        let test_ethplorer_api_key = settings
            .get::<String>("test_ethplorer")
            .unwrap_or_else(|_| panic!("test ethplorer key is not set in Settings.toml, exit."));
        let decimal = get_token_decimal(&test_ethplorer_api_key, erc20_contract_address).await;
        assert_eq!(decimal, 18);
    }

    #[should_panic(
        expected = "Error on fetching decimals for token contract 0x0121212121212121212121212212121212121212"
    )]
    #[tokio::test]
    async fn get_token_decimal_fail() {
        // non existent token address
        let erc20_contract_address = "0x0121212121212121212121212212121212121212";
        let mut settings = config::Config::default();
        settings.merge(config::File::with_name("Settings")).unwrap();
        let test_ethplorer_api_key = settings
            .get::<String>("test_ethplorer")
            .unwrap_or_else(|_| panic!("test ethplorer key is not set in Settings.toml, exit."));
        get_token_decimal(&test_ethplorer_api_key, erc20_contract_address).await;
    }

    #[tokio::test]
    async fn get_token_price_success() {
        let erc20_token_id = "yearn-finance";
        let price = get_token_price(erc20_token_id, "usd", true).await;
        let price_eth = get_token_price(erc20_token_id, "eth", true).await;
        assert_ne!(price, 0.0);
        assert_ne!(price_eth, 0.0);
    }

    #[tokio::test]
    async fn get_token_price_fail() {
        let price = get_token_price("nonexistingtoken", "usd", true).await;
        let price_eth = get_token_price("nonexistingtoken", "eth", true).await;
        assert_eq!(price, 0.0);
        assert_eq!(price_eth, 0.0);
    }

    #[tokio::test]
    async fn get_erc20_balance_for_account_success() {
        let test_account_address: H160 =
            "000000000000000000000000000000000000dead".parse().unwrap();
        let test_contract_address = "0x98b2dE885E916b598f65DeD2fDbb63187EAEf184";
        let mut settings = config::Config::default();
        settings.merge(config::File::with_name("Settings")).unwrap();
        let test_etherscan_api_key = settings
            .get::<String>("test_etherscan")
            .unwrap_or_else(|_| panic!("test etherscan key is not set in Settings.toml, exit."));
        let test_ethplorer_api_key = settings
            .get::<String>("test_ethplorer")
            .unwrap_or_else(|_| panic!("test ethplorer key is not set in Settings.toml, exit."));
        let balance = get_erc20_balance_for_account(
            test_account_address,
            &test_etherscan_api_key,
            &test_ethplorer_api_key,
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
        let test_etherscan_api_key = settings
            .get::<String>("test_etherscan")
            .unwrap_or_else(|_| panic!("test etherscan key is not set in Settings.toml, exit."));
        let test_ethplorer_api_key = settings
            .get::<String>("test_ethplorer")
            .unwrap_or_else(|_| panic!("test ethplorer key is not set in Settings.toml, exit."));
        get_erc20_balance_for_account(
            test_account_address,
            &test_etherscan_api_key,
            &test_ethplorer_api_key,
            test_contract_address,
        )
        .await;
    }
}
