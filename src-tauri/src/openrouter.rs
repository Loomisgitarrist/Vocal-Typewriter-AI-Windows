use serde::{Deserialize, Serialize};
use reqwest::Client;

#[derive(Debug, Serialize, Deserialize)]
pub struct Pricing {
    pub prompt: String,
    pub completion: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Model {
    pub id: String,
    pub name: String,
    pub pricing: Pricing,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    pub data: Vec<Model>,
}

#[tauri::command]
pub async fn fetch_openrouter_models() -> Result<Response, String> {
    let client = Client::new();
    let response = client
        .get("https://openrouter.ai/api/v1/models")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if response.status().is_success() {
        let models: Response = response.json().await.map_err(|e| e.to_string())?;
        Ok(models)
    } else {
        Err(format!("Request failed with status: {}", response.status()))
    }
}
