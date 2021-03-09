use serde_json::Value;
use std::error;
use std::io;
use std::thread::sleep;
use std::time::Duration;

pub async fn fetch(url: &String, verbose: bool) -> Result<Value, Box<dyn error::Error>> {
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

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn fetch_success() {
        // AAVE token address
        let from_contract_address = "0x7fc66500c84a76ad7e9c93437bfc5ac33e2ddae9";

        // USDT token address
        let to_contract_address = "0xdac17f958d2ee523a2206206994597c13d831ec7";

        let amount = "1000000000000";

        let url = format!(
            "https://api.paraswap.io/v2/prices/?from={}&to={}&amount={}",
            from_contract_address, to_contract_address, amount
        );

        let result = fetch(&url, false).await;
        assert_eq!(result.unwrap().is_object(), true);
    }

    #[tokio::test]
    async fn fetch_non_existent_token_fail() {
        // non existent token address
        let from_contract_address = "0x0121212121212121212121212212121212121212";

        // USDT token address
        let to_contract_address = "0xdac17f958d2ee523a2206206994597c13d831ec7";

        let amount = "1000000000000";

        let url = format!(
            "https://api.paraswap.io/v2/prices/?from={}&to={}&amount={}",
            from_contract_address, to_contract_address, amount
        );

        let result = fetch(&url, false).await;
        assert_eq!(
            result.unwrap().to_string(),
            "{\"error\":\"Token not found\"}"
        );
    }
}
