use anyhow;
use serde::{de::DeserializeOwned, Serialize};
use std::{fs, path::PathBuf};

extern crate serde_yaml;
pub trait Config {
    fn load(&mut self) -> anyhow::Result<()>
    where
        Self: DeserializeOwned;
    fn save(&mut self) -> anyhow::Result<()>
    where
        Self: Serialize,
    {
        let config = serde_yaml::to_string(&self)?;
        fs::write(&self.get_config_file(), config)?;
        Ok(())
    }
    fn get_config_file(&self) -> PathBuf;
    fn set_config_file(&mut self, path: PathBuf);
    fn print(&self);
}

pub fn config<C: Config>(cfg: &mut C, config_file: Option<&PathBuf>) -> anyhow::Result<()>
where
    C: DeserializeOwned,
{
    if let Some(c) = config_file {
        cfg.set_config_file(c.to_path_buf());
    }
    cfg.load()?;
    Ok(())
}
