use serde_json::Value;
use std::error;
use std::io;
use std::thread::sleep;
use std::time::Duration;

#[allow(dead_code)]
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

#[allow(dead_code)]
pub async fn get_token_price(
    from_contract_address: &str,
    versus_name: &str,
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
        "https://api.paraswap.io/v2/prices/?from={}&to={}&amount={}",
        from_contract_address, to_contract_address, amount
    );
    let json = fetch(&url, verbose).await?;

    let mix_selector = Some(r#""priceRoute"."bestRoute"[0]."amount""#);

    let value: Value = jql::walker(&json, mix_selector)?;

    Ok(value
        .as_str()
        .unwrap_or("0.0")
        .parse::<f64>()
        .unwrap_or(0.0))
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

    #[tokio::test]
    async fn get_token_price_success() {
        // AAVE token address
        let contract_address = "0x7fc66500c84a76ad7e9c93437bfc5ac33e2ddae9";
        let price = get_token_price(contract_address, "usd", true)
            .await
            .unwrap();
        assert_ne!(price, 0.0);
    }

    #[tokio::test]
    async fn get_token_price_fail() {
        // non existent token address
        let contract_address = "0x0121212121212121212121212212121212121212";

        let result = get_token_price(contract_address, "usd", true).await;
        if let Result::Err(err) = result {
            assert_eq!(
                (*err).to_string(),
                "Node \"priceRoute\" not found on the parent element"
            );
        }
    }
}
