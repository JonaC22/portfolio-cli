use super::price_provider;
use async_trait::async_trait;
use serde_json::Value;
use std::error;
use std::io;
use std::thread::sleep;
use std::time::Duration;

pub struct Paraswap;

#[async_trait]
impl price_provider::PriceProvider for Paraswap {
    #[allow(dead_code)]
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
                            format!("Could not fetch from paraswap: response body: {:?}", &body),
                        )));
                    } else {
                        retry += 1;
                        if verbose {
                            println!(
                                "Failed to fetch from paraswap, retry up to {}, retry number: {}",
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

    #[allow(dead_code)]
    async fn get_token_price<'a>(
        &self,
        from_contract_address: &'a str,
        versus_name: &'a str,
        verbose: bool,
    ) -> Result<f64, Box<dyn error::Error>> {
        let to_contract_address;

        match versus_name {
            "eth" => to_contract_address = "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
            "usd" => to_contract_address = "0xdac17f958d2ee523a2206206994597c13d831ec7", // USDT token address
            _ => {
                return Err(Box::new(io::Error::new(
                    io::ErrorKind::ConnectionRefused,
                    format!("Could not fetch token price versus {}", versus_name),
                )))
            }
        }

        let amount = "1000000000000";

        let url = format!(
            "https://apiv5.paraswap.io/prices/?srcToken={}&destToken={}&amount={}&network=1",
            from_contract_address, to_contract_address, amount
        );
        let json = self.fetch(&url, verbose).await?;
        let mix_selector = r#""priceRoute"."destAmount""#;

        let value: Value = jql::walker(&json, mix_selector)?;

        Ok(value
            .as_str()
            .unwrap_or("0.0")
            .parse::<f64>()
            .unwrap_or(0.0))
    }
}
#[cfg(test)]
mod test {
    use crate::lib::paraswap::Paraswap;
    use crate::lib::price_provider::PriceProvider;

    #[tokio::test]
    async fn fetch_success() {
        let paraswap = Paraswap;

        // AAVE token address
        let from_contract_address = "0x7fc66500c84a76ad7e9c93437bfc5ac33e2ddae9";

        // USDT token address
        let to_contract_address = "0xdac17f958d2ee523a2206206994597c13d831ec7";

        let amount = "1000000000000";

        let url = format!(
            "https://apiv5.paraswap.io/prices/?srcToken={}&destToken={}&amount={}",
            from_contract_address, to_contract_address, amount
        );

        let result = paraswap.fetch(&url, false).await;
        assert!(result.unwrap().is_object());
    }

    #[tokio::test]
    async fn fetch_non_existent_token_fail() {
        let paraswap = Paraswap;

        // non existent token address
        let from_contract_address = "0x0121212121212121212121212212121212121212";

        // USDT token address
        let to_contract_address = "0xdac17f958d2ee523a2206206994597c13d831ec7";

        let amount = "1000000000000";

        let url = format!(
            "https://apiv5.paraswap.io/prices/?srcToken={}&destToken={}&amount={}&network=1",
            from_contract_address, to_contract_address, amount
        );

        let result = paraswap.fetch(&url, false).await;
        assert_eq!(
            result.unwrap().to_string(),
            "{\"error\":\"Token not found\"}"
        );
    }

    #[tokio::test]
    async fn get_token_price_success() {
        let paraswap = Paraswap;

        // AAVE token address
        let contract_address = "0x7fc66500c84a76ad7e9c93437bfc5ac33e2ddae9";
        let price = paraswap
            .get_token_price(contract_address, "usd", true)
            .await
            .unwrap();
        assert_ne!(price, 0.0);
    }

    #[tokio::test]
    async fn get_token_price_fail() {
        let paraswap = Paraswap;

        // non existent token address
        let contract_address = "0x0121212121212121212121212212121212121212";

        let result = paraswap
            .get_token_price(contract_address, "usd", true)
            .await;
        if let Result::Err(err) = result {
            assert_eq!(
                (*err).to_string(),
                "Node \"priceRoute\" not found on the parent element"
            );
        }
    }
}
