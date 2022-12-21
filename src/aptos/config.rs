use crate::config::Config as cfg;
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
impl cfg for Config {
    fn init(&self) -> anyhow::Result<()> {
        let config = serde_yaml::to_string(&self)?;
        fs::write(&self.get_config_file(), config)?;
        Ok(())
    }
    fn get_config_file(&self) -> PathBuf {
        self.scale_config_file.clone()
    }
    fn set_config_file(&mut self, path: PathBuf) {
        self.scale_config_file = path;
    }
    fn print(&self) {
        println!("{:?}", self)
    }
}
