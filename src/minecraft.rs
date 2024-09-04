use eyre::{bail, Result};

use crate::{
    config::MinecraftConfig,
    utils::{is_valid_minecraft_username, is_valid_username},
};

pub async fn username_to_id(username: &str) -> Result<String> {
    let url = format!(
        "https://api.mojang.com/users/profiles/minecraft/{}",
        username
    );
    let response = reqwest::get(&url).await?;
    let response = response.json::<serde_json::Value>().await?;
    let uuid = response["id"]
        .as_str()
        .ok_or_else(|| eyre::eyre!("Invalid response"))?;
    Ok(uuid.to_string())
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct Response {
    message: String,
}

const ALREADY_WHITELISTED: &str = "Player is already whitelisted";
const PLAYER_ADDED_PREFIX: &str = "Added ";
const PLAYER_ADDED_SUFFIC: &str = " to the whitelist";

pub async fn whitelist_add(username: &str, config: &MinecraftConfig) -> Result<()> {
    let username = username.to_lowercase();
    if !is_valid_minecraft_username(&username) {
        return Err(eyre::eyre!("Invalid username"));
    }

    let client = reqwest::Client::new();
    let res = client
        .post(format!("{}/command", config.rcon_api))
        .header(
            "Authorization",
            format!("Bearer {}", config.rcon_api_password),
        )
        .json(&serde_json::json!({
            "command": format!("whitelist add {}", username)
        }))
        .send()
        .await?;

    let res = res.json::<Response>().await?;

    if res.message == ALREADY_WHITELISTED || res.message.starts_with(PLAYER_ADDED_PREFIX) {
        return Ok(());
    }

    bail!("whitelist_add: {:?}", res);
}

pub async fn whitelist_remove(username: &str, config: &MinecraftConfig) -> Result<()> {
    let username = username.to_lowercase();
    if !is_valid_minecraft_username(&username) {
        return Err(eyre::eyre!("Invalid username"));
    }

    let client = reqwest::Client::new();
    let res = client
        .post(format!("{}/command", config.rcon_api))
        .header(
            "Authorization",
            format!("Bearer {}", config.rcon_api_password),
        )
        .json(&serde_json::json!({
            "command": format!("whitelist remove {}", username)
        }))
        .send()
        .await?;

    let _ = res.json::<Response>().await?;
    Ok(())
}
