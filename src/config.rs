use anyhow;
use log::debug;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, str::FromStr};
extern crate serde_yaml;
pub trait Config {
    fn load(&mut self) -> anyhow::Result<()>;
    fn init(&mut self) -> anyhow::Result<()>;
    fn save(&mut self) -> anyhow::Result<()> {
        let config = serde_yaml::to_string(&self)?;
        fs::write(&self.get_config_file(), config)?;
        Ok(())
    }
    fn get_config_file(&self) -> PathBuf;
    fn set_config_file(&mut self, path: PathBuf);
    fn print(&self);
}

pub fn config<C: Config + Serialize + Deserialize>(
    cfg: &mut C,
    config_file: Option<&PathBuf>,
) -> anyhow::Result<()> {
    if let Some(c) = config_file {
        cfg.set_config_file(c.to_path_buf());
    }
    let config_file = cfg.get_config_file();
    match cfg.load() {
        Ok(_) => {
            debug!("Config file loaded: {:?}", config_file);
        }
        Err(e) => {
            debug!(
                "Config file not load: {:?}, Attempt to initialize",
                config_file
            );
            cfg.init()?;
        }
    }
    Ok(())
}
