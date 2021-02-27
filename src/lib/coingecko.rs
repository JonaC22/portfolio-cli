use serde_json::{Error, Value};
use std::thread::sleep;
use std::time::Duration;

pub async fn fetch(url: &String, verbose: bool) -> Result<Value, String> {
    let mut retry: u32 = 0;
    let max_retries: u32 = 5;

    loop {
        let body = reqwest::get(url).await.unwrap().text().await.unwrap();
        let result: std::result::Result<Value, Error> = serde_json::from_str(&body);

        match result {
            Ok(json) => return Ok(json),
            _ => {
                if retry > max_retries {
                    return Err(format!(
                        "Could not fetch from coingecko: response body: {:?}",
                        &body
                    ));
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

pub async fn get_token_id_from_contract_address(
    contract_address: &str,
    verbose: bool,
) -> Result<String, String> {
    let url = format!(
        "https://api.coingecko.com/api/v3/coins/ethereum/contract/{}",
        contract_address
    );
    let json = fetch(&url, verbose).await?;

    let mix_selector = Some(r#""id""#);

    let value: Value = jql::walker(&json, mix_selector).unwrap_or_default();

    Ok(value
        .as_str()
        .ok_or_else(|| "")
        .unwrap_or_else(|_e| "")
        .to_string())
}

pub async fn get_token_price(
    token_id: &str,
    versus_name: &str,
    verbose: bool,
) -> Result<f64, String> {
    let url = format!(
        "https://api.coingecko.com/api/v3/simple/price?ids={}&vs_currencies={}",
        token_id, versus_name
    );
    let json = fetch(&url, verbose).await?;

    let selector = format!(r#""{}"."{}""#, token_id, versus_name);
    let mix_selector = Some(selector.as_str());

    let value: Value = jql::walker(&json, mix_selector).unwrap_or_default();

    Ok(value.as_f64().ok_or_else(|| 0.0).unwrap_or_else(|_e| 0.0))
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn fetch_success() {
        // YFI token address
        let erc20_contract_address = "0x0bc529c00C6401aEF6D220BE8C6Ea1667F6Ad93e";

        let url = format!(
            "https://api.coingecko.com/api/v3/coins/ethereum/contract/{}",
            erc20_contract_address
        );

        let result = fetch(&url, false).await;
        assert_eq!(result.unwrap().is_object(), true);
    }

    #[tokio::test]
    async fn fetch_non_existent_token_fail() {
        // non existent token address
        let erc20_contract_address = "0x0121212121212121212121212212121212121212";

        let url = format!(
            "https://api.coingecko.com/api/v3/coins/ethereum/contract/{}",
            erc20_contract_address
        );

        let result = fetch(&url, false).await;
        assert_eq!(
            result.unwrap().to_string(),
            "{\"error\":\"Could not find coin with the given id\"}"
        );
    }

    #[tokio::test]
    async fn get_token_id_success() {
        // YFI token address
        let erc20_contract_address = "0x0bc529c00C6401aEF6D220BE8C6Ea1667F6Ad93e";
        let id = get_token_id_from_contract_address(erc20_contract_address, true)
            .await
            .unwrap();
        assert_eq!(id, "yearn-finance");
    }

    #[tokio::test]
    async fn get_token_id_fail() {
        // non existent token address
        let erc20_contract_address = "0x0121212121212121212121212212121212121212";
        let id = get_token_id_from_contract_address(erc20_contract_address, true)
            .await
            .unwrap();
        assert_eq!(id, "");
    }

    #[tokio::test]
    async fn get_token_price_success() {
        let erc20_token_id = "yearn-finance";
        let price = get_token_price(erc20_token_id, "usd", true).await.unwrap();
        let price_eth = get_token_price(erc20_token_id, "eth", true).await.unwrap();
        assert_ne!(price, 0.0);
        assert_ne!(price_eth, 0.0);
    }

    #[tokio::test]
    async fn get_token_price_fail() {
        let price = get_token_price("nonexistingtoken", "usd", true)
            .await
            .unwrap();
        let price_eth = get_token_price("nonexistingtoken", "eth", true)
            .await
            .unwrap();
        assert_eq!(price, 0.0);
        assert_eq!(price_eth, 0.0);
    }
}
