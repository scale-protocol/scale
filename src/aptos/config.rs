use crate::config::Config as cfg;
extern crate serde;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub scale_config_file: PathBuf,
}
impl Default for Config {
    fn default() -> Self {
        let home_dir = match home::home_dir() {
            Some(p) => p,
            None => PathBuf::from("/tmp/"),
        };
        let scale_home_dir = home_dir.join(".scale");
        if !scale_home_dir.is_dir() {
            fs::create_dir(&scale_home_dir).unwrap();
        }
        Config {
            scale_config_file: scale_home_dir.join("aptos_config.yaml"),
        }
    }
}
// impl cfg for Config {

// }
