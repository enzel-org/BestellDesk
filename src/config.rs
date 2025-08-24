use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LocalConfig {
    pub mongo_uri: Option<String>,
    pub remember_server: bool,
    pub client_id: Option<String>, 
}

fn config_path() -> anyhow::Result<PathBuf> {
    let dirs = ProjectDirs::from("dev", "BestellDesk", "BestellDesk")
        .ok_or_else(|| anyhow::anyhow!("cannot resolve config dir"))?;
    let file = dirs.config_dir().join("config.json");
    fs::create_dir_all(dirs.config_dir())?;
    Ok(file)
}

pub fn load() -> anyhow::Result<LocalConfig> {   // <-- pub
    let p = config_path()?;
    if !p.exists() {
        return Ok(LocalConfig::default());
    }
    let bytes = fs::read(p)?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn save(cfg: &LocalConfig) -> anyhow::Result<()> {   // <-- pub
    let p = config_path()?;
    let bytes = serde_json::to_vec_pretty(cfg)?;
    fs::write(p, bytes)?;
    Ok(())
}
