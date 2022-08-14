use async_trait::async_trait;
use serde_json::Value;
use std::error;

#[async_trait]
pub trait PriceProvider {
    async fn fetch<'a>(&self, url: &'a str, verbose: bool) -> Result<Value, Box<dyn error::Error>>;

    async fn get_token_id_from_contract_address<'a>(
        &self,
        contract_address: &'a str,
        verbose: bool,
    ) -> Result<String, Box<dyn error::Error>>;

    async fn get_token_price<'a>(
        &self,
        token_id: &'a str,
        versus_name: &'a str,
        verbose: bool,
    ) -> Result<f64, Box<dyn error::Error>>;
}
