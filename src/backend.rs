use anyhow::Result;

#[derive(Debug)]
pub struct Config {
    pub remote: String,
    pub user: String,
    pub host: String,
}

pub trait Backend {
    fn read_config(&self) -> Result<Option<Config>>;
    fn write_config(&self, config: &Config) -> Result<()>;
}
