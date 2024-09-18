use crate::config::MinecraftConfig;
use eyre::Result;

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct MinecraftPlayer {
    pub name: String,
    pub id: String,
}

pub async fn connected_players(config: &MinecraftConfig) -> Result<Vec<MinecraftPlayer>> {
    let client = reqwest::Client::new();
    let res = client
        .get(format!("{}/players", config.restadmin_url))
        .header(
            "Authorization",
            format!("Bearer {}", config.restadmin_token),
        )
        .send()
        .await?;

    Ok(res.json::<Vec<MinecraftPlayer>>().await?)
}

pub async fn whitelist_add(username: &str, config: &MinecraftConfig) -> Result<MinecraftPlayer> {
    let username = username.to_lowercase();

    let client = reqwest::Client::new();
    let res = client
        .post(format!("{}/whitelist/{username}", config.restadmin_url))
        .header(
            "Authorization",
            format!("Bearer {}", config.restadmin_token),
        )
        .send()
        .await?;

    Ok(res.json::<MinecraftPlayer>().await?)
}

pub async fn whitelist_remove(
    username_or_uuid: &str,
    config: &MinecraftConfig,
) -> Result<MinecraftPlayer> {
    let username = username_or_uuid.to_lowercase();

    let client = reqwest::Client::new();
    let res = client
        .delete(format!("{}/whitelist/{username}", config.restadmin_url))
        .header(
            "Authorization",
            format!("Bearer {}", config.restadmin_token),
        )
        .send()
        .await?;

    Ok(res.json::<MinecraftPlayer>().await?)
}
