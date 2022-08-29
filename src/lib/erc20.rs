use crate::coingecko::Coingecko;
use crate::lib::price_provider::PriceProvider;
use crate::paraswap::Paraswap;
use governor::{Quota, RateLimiter};
use indicatif::ProgressBar;
use nonzero_ext::*;
use serde_json::Value;
use std::collections::HashMap;
use std::convert::TryInto;
use std::error;
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

#[derive(Debug)]
pub struct ListConfig {
    pub startblock: i32,
    pub endblock: i32,
    pub show_progress_bar: bool,
    pub verbose: bool,
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

impl ListConfig {
    pub fn new(
        startblock: Option<i32>,
        endblock: Option<i32>,
        show_progress_bar: bool,
        verbose: bool,
    ) -> ListConfig {
        let mut startblock_number = 0;
        if let Some(n) = startblock {
            startblock_number = n;
        }

        let mut endblock_number = 999999999;
        if let Some(n) = endblock {
            endblock_number = n;
        }

        ListConfig {
            startblock: startblock_number,
            endblock: endblock_number,
            show_progress_bar,
            verbose,
        }
    }
}

pub type Tokens = HashMap<String, Option<TokenInfo>>;

pub async fn get_token_decimal(
    ethplorer_api_key: &str,
    contract_address: &str,
) -> Result<u32, Box<dyn error::Error>> {
    let url = format!(
        "https://api.ethplorer.io/getTokenInfo/{}?apiKey={}
    ",
        contract_address, ethplorer_api_key
    );
    let body = reqwest::get(&url).await?.text().await?;
    let json: Value = serde_json::from_str(&body)?;
    let mix_selector = r#""decimals""#;

    let results = jql::walker(&json, mix_selector)?;

    match results {
        Value::String(value) => Ok(value.parse::<u32>()?),
        Value::Number(value) => Ok(value.to_string().parse::<u32>()?),
        _ => Err(Box::new(io::Error::new(
            io::ErrorKind::ConnectionRefused,
            format!(
                "Error on fetching decimals for token contract {}",
                contract_address
            ),
        ))),
    }
}

pub async fn get_erc20_balance_for_account(
    account_address: H160,
    etherscan_api_key: &str,
    ethplorer_api_key: &str,
    contract_address: &str,
) -> Result<f64, Box<dyn error::Error>> {
    let url = format!("https://api.etherscan.io/api?module=account&action=tokenbalance&contractaddress={}&address={:?}&tag=latest&apikey={}", contract_address, account_address, etherscan_api_key);
    let body = reqwest::get(&url).await?.text().await?;
    let json: Value = serde_json::from_str(&body)?;
    let mix_selector = r#""result""#;
    let message_selector = r#""message""#;

    let message = jql::walker(&json, message_selector)?;
    if let Value::String(status) = message {
        if &status != "OK" {
            return Err(Box::new(io::Error::new(
                io::ErrorKind::ConnectionRefused,
                format!("Error on processing ERC20 balance for {}", contract_address),
            )));
        }
    }

    let results = jql::walker(&json, mix_selector)?;

    let decimal = get_token_decimal(ethplorer_api_key, contract_address).await?;

    match results {
        Value::String(value) => Ok(value.parse::<f64>()? / 10_u64.pow(decimal) as f64),
        _ => Err(Box::new(io::Error::new(
            io::ErrorKind::ConnectionRefused,
            format!("Error on processing ERC20 balance for {}", contract_address),
        ))),
    }
}

pub async fn list_erc20_for_account(
    account_address: H160,
    etherscan_api_key: &str,
    ethplorer_api_key: &str,
    list_config: ListConfig,
) -> Result<Tokens, Box<dyn error::Error>> {
    let price_providers: Vec<Box<dyn PriceProvider>> =
        vec![Box::new(Coingecko), Box::new(Paraswap)];
    let url =
        format!("http://api.etherscan.io/api?module=account&action=tokentx&address={:?}&startblock={}&endblock={}&sort=asc&apikey={}", account_address, list_config.startblock, list_config.endblock, etherscan_api_key);
    let body = reqwest::get(&url).await?.text().await?;
    let json: Value = serde_json::from_str(&body)?;
    let mix_selector = r#""result"|{"tokenSymbol", "tokenName", "contractAddress"}"#;

    let message_selector = r#""message""#;

    let message = jql::walker(&json, message_selector)?;
    if let Value::String(status) = message {
        if &status != "OK" {
            return Err(Box::new(io::Error::new(
                io::ErrorKind::ConnectionRefused,
                "Error on processing the list of ERC20 tokens",
            )));
        }
    }
    let results = jql::walker(&json, mix_selector)?;

    let limiter = RateLimiter::direct(Quota::per_second(nonzero!(8u32))); // Allow 8 units per second

    match results {
        Value::Array(value) => {
            let mut tokens = Tokens::new();
            let mut pb: Option<ProgressBar> = None;
            if list_config.show_progress_bar {
                pb = Some(ProgressBar::new(value.len().try_into()?));
            }

            for entry in value {
                if let Some(ref p) = pb {
                    p.inc(1);
                }
                io::stdout().flush()?;
                let token_symbol: String = entry
                    .get("tokenSymbol")
                    .ok_or("tokenSymbol not present")?
                    .to_string();

                match tokens.get(&token_symbol) {
                    None => {
                        let contract_address: &str = entry
                            .get("contractAddress")
                            .ok_or("contractAddress not present")?
                            .as_str()
                            .ok_or("contractAddress invalid")?;

                        let token_id_result = match get_token_id_from_contract_address(
                            &price_providers,
                            contract_address,
                            &list_config,
                        )
                        .await
                        {
                            Some(value) => value,
                            None => continue,
                        };

                        let token_id = token_id_result?;

                        let balance: f64 = get_erc20_balance_for_account(
                            account_address,
                            etherscan_api_key,
                            ethplorer_api_key,
                            contract_address,
                        )
                        .await?;

                        let token_usd_price_future = price_providers[0].get_token_price(
                            &token_id,
                            "usd",
                            list_config.verbose,
                        );
                        match limiter.check() {
                            Ok(()) => (),
                            _ => sleep(Duration::from_millis(2000)),
                        }

                        let token_eth_price_future = price_providers[0].get_token_price(
                            &token_id,
                            "eth",
                            list_config.verbose,
                        );
                        match limiter.check() {
                            Ok(()) => (),
                            _ => sleep(Duration::from_millis(2000)),
                        }

                        let usd_price = match token_usd_price_future.await {
                            Ok(v) => v,
                            Err(_) => continue,
                        };

                        let eth_price = match token_eth_price_future.await {
                            Ok(v) => v,
                            Err(_) => continue,
                        };

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
            if let Some(ref p) = pb {
                p.finish_with_message("done")
            }
            Ok(tokens)
        }
        _ => Err(Box::new(io::Error::new(
            io::ErrorKind::ConnectionRefused,
            "Error on processing the list of ERC20 tokens",
        ))),
    }
}

async fn get_token_id_from_contract_address(
    price_providers: &Vec<Box<dyn PriceProvider>>,
    contract_address: &str,
    list_config: &ListConfig,
) -> Option<Result<String, Box<dyn error::Error>>> {
    for price_provider in price_providers {
        let token_id_result = price_provider
            .get_token_id_from_contract_address(contract_address, list_config.verbose)
            .await;
        if let Result::Err(_err) = token_id_result {
            continue;
        }
        return Some(token_id_result);
    }

    None
}

#[cfg(test)]
mod test {
    use super::*;
    use config;
    use web3::types::H160;

    #[tokio::test]
    async fn get_token_decimal_success() {
        // YFI token address
        let erc20_contract_address = "0x0bc529c00C6401aEF6D220BE8C6Ea1667F6Ad93e";
        let config_builder = config::Config::builder()
            .add_source(config::File::new("Settings.toml", config::FileFormat::Toml));
        let settings = config_builder.build().unwrap();
        let test_ethplorer_api_key = settings
            .get::<String>("test_ethplorer")
            .unwrap_or_else(|_| panic!("test ethplorer key is not set in Settings.toml, exit."));
        let decimal = get_token_decimal(&test_ethplorer_api_key, erc20_contract_address)
            .await
            .unwrap();
        assert_eq!(decimal, 18);
    }

    #[tokio::test]
    async fn get_token_decimal_fail() {
        // non existent token address
        let erc20_contract_address = "0x0121212121212121212121212212121212121212";
        let config_builder = config::Config::builder()
            .add_source(config::File::new("Settings.toml", config::FileFormat::Toml));
        let settings = config_builder.build().unwrap();
        let test_ethplorer_api_key = settings
            .get::<String>("test_ethplorer")
            .unwrap_or_else(|_| panic!("test ethplorer key is not set in Settings.toml, exit."));
        let decimal = get_token_decimal(&test_ethplorer_api_key, erc20_contract_address).await;

        if let Result::Err(err) = decimal {
            assert_eq!(
                (*err).to_string(),
                "Node \"decimals\" not found on the parent element"
            );
        }
    }

    #[tokio::test]
    async fn get_erc20_balance_for_account_success() {
        let test_account_address: H160 =
            "000000000000000000000000000000000000dead".parse().unwrap();
        let test_contract_address = "0x98b2dE885E916b598f65DeD2fDbb63187EAEf184";
        let config_builder = config::Config::builder()
            .add_source(config::File::new("Settings.toml", config::FileFormat::Toml));
        let settings = config_builder.build().unwrap();
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
        .await
        .unwrap();
        assert_ne!(balance, 0.0);
    }

    #[tokio::test]
    async fn get_erc20_balance_for_account_fail() {
        let test_account_address: H160 =
            "000000000000000000000000000000000000dead".parse().unwrap();
        let test_contract_address = "0x98b2dE885E916b598f65DeD2";
        let config_builder = config::Config::builder()
            .add_source(config::File::new("Settings.toml", config::FileFormat::Toml));
        let settings = config_builder.build().unwrap();
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
        if let Result::Err(err) = balance {
            assert_eq!(
                (*err).to_string(),
                "Error on processing ERC20 balance for 0x98b2dE885E916b598f65DeD2"
            );
        }
    }

    #[tokio::test]
    async fn list_erc20_for_account_success() {
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

        let list_config = ListConfig::new(Some(11855520), Some(11855590), false, false);

        let list_erc20 = list_erc20_for_account(
            test_account_address,
            &test_etherscan_api_key,
            &test_ethplorer_api_key,
            list_config,
        )
        .await
        .unwrap();

        assert_eq!(list_erc20.len(), 1);
    }

    #[tokio::test]
    async fn list_erc20_for_account_fail() {
        let test_account_address: H160 = "0x0121212121212121212121212212121212121212"
            .parse()
            .unwrap();
        let config_builder = config::Config::builder()
            .add_source(config::File::new("Settings.toml", config::FileFormat::Toml));
        let settings = config_builder.build().unwrap();
        let test_etherscan_api_key = settings
            .get::<String>("test_etherscan")
            .unwrap_or_else(|_| panic!("test etherscan key is not set in Settings.toml, exit."));
        let test_ethplorer_api_key = settings
            .get::<String>("test_ethplorer")
            .unwrap_or_else(|_| panic!("test ethplorer key is not set in Settings.toml, exit."));

        let list_config = ListConfig::new(Some(11855520), Some(11855590), false, false);

        let list_erc20 = list_erc20_for_account(
            test_account_address,
            &test_etherscan_api_key,
            &test_ethplorer_api_key,
            list_config,
        )
        .await;

        if let Result::Err(err) = list_erc20 {
            assert_eq!(
                (*err).to_string(),
                "Error on processing the list of ERC20 tokens"
            );
        }
    }
}
