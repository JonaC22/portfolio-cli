use super::price_provider;
use async_trait::async_trait;
use serde_json::Value;
use std::error;
use std::io;
use std::thread::sleep;
use std::time::Duration;

pub struct Coingecko;

#[async_trait]
impl price_provider::PriceProvider for Coingecko {
    async fn fetch<'a>(&self, url: &'a str, verbose: bool) -> Result<Value, Box<dyn error::Error>> {
        let mut retry: u32 = 0;
        let max_retries: u32 = 5;

        loop {
            let body = reqwest::get(url).await?.text().await?;
            let result = serde_json::from_str(&body);

            match result {
                Ok(json) => return Ok(json),
                _ => {
                    if retry > max_retries {
                        return Err(Box::new(io::Error::new(
                            io::ErrorKind::ConnectionRefused,
                            format!("Could not fetch from coingecko: response body: {:?}", &body),
                        )));
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

    async fn get_token_id_from_contract_address<'a>(
        &self,
        contract_address: &'a str,
        verbose: bool,
    ) -> Result<String, Box<dyn error::Error>> {
        let url = format!(
            "https://api.coingecko.com/api/v3/coins/ethereum/contract/{}",
            contract_address
        );
        let json = self.fetch(&url, verbose).await?;

        let mix_selector = r#""id""#;

        let value = jql::walker(&json, mix_selector)?;

        Ok(value.as_str().ok_or("").unwrap_or("").to_string())
    }

    async fn get_token_price<'a>(
        &self,
        token_id: &'a str,
        versus_name: &'a str,
        verbose: bool,
    ) -> Result<f64, Box<dyn error::Error>> {
        let url = format!(
            "https://api.coingecko.com/api/v3/simple/price?ids={}&vs_currencies={}",
            token_id, versus_name
        );
        let json = self.fetch(&url, verbose).await?;

        let selector = format!(r#""{}"."{}""#, token_id, versus_name);
        let mix_selector = selector.as_str();

        let value: Value = jql::walker(&json, mix_selector)?;

        Ok(value.as_f64().ok_or(0.0).unwrap_or(0.0))
    }
}

#[cfg(test)]
mod test {
    use crate::lib::coingecko::Coingecko;
    use crate::lib::price_provider::PriceProvider;

    #[tokio::test]
    async fn fetch_success() {
        let coingecko = Coingecko;

        // YFI token address
        let erc20_contract_address = "0x0bc529c00C6401aEF6D220BE8C6Ea1667F6Ad93e";

        let url = format!(
            "https://api.coingecko.com/api/v3/coins/ethereum/contract/{}",
            erc20_contract_address
        );

        let result = coingecko.fetch(&url, false).await;
        assert!(result.unwrap().is_object());
    }

    #[tokio::test]
    async fn fetch_non_existent_token_fail() {
        let coingecko = Coingecko;

        // non existent token address
        let erc20_contract_address = "0x0121212121212121212121212212121212121212";

        let url = format!(
            "https://api.coingecko.com/api/v3/coins/ethereum/contract/{}",
            erc20_contract_address
        );

        let result = coingecko.fetch(&url, false).await;
        assert_eq!(
            result.unwrap().to_string(),
            "{\"error\":\"Could not find coin with the given id\"}"
        );
    }

    #[tokio::test]
    async fn get_token_id_success() {
        let coingecko = Coingecko;

        // YFI token address
        let erc20_contract_address = "0x0bc529c00C6401aEF6D220BE8C6Ea1667F6Ad93e";
        let id = coingecko
            .get_token_id_from_contract_address(erc20_contract_address, true)
            .await
            .unwrap();
        assert_eq!(id, "yearn-finance");
    }

    #[tokio::test]
    async fn get_token_id_fail() {
        let coingecko = Coingecko;
        // non existent token address
        let erc20_contract_address = "0x0121212121212121212121212212121212121212";
        let result = coingecko
            .get_token_id_from_contract_address(erc20_contract_address, true)
            .await;
        if let Result::Err(err) = result {
            assert_eq!(
                (*err).to_string(),
                "Node \"id\" not found on the parent element"
            );
        }
    }

    #[tokio::test]
    async fn get_token_price_success() {
        let coingecko = Coingecko;
        let erc20_token_id = "yearn-finance";
        let price = coingecko
            .get_token_price(erc20_token_id, "usd", true)
            .await
            .unwrap();
        let price_eth = coingecko
            .get_token_price(erc20_token_id, "eth", true)
            .await
            .unwrap();
        assert_ne!(price, 0.0);
        assert_ne!(price_eth, 0.0);
    }

    #[tokio::test]
    async fn get_token_price_fail() {
        let coingecko = Coingecko;
        let result = coingecko
            .get_token_price("nonexistingtoken", "usd", true)
            .await;
        if let Result::Err(err) = result {
            assert_eq!(
                (*err).to_string(),
                "Node \"nonexistingtoken\" not found on the parent element"
            );
        }
    }
}
