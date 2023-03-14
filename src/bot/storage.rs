use crate::bot::state::{Address, State,Position};
use crate::com;
use sled::{Batch, Db};
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub enum Prefix {
    Active = 1,
    History,
    None,
}
#[derive(Clone)]
pub struct Keys {
    keys: Vec<String>,
}

impl Keys {
    pub fn new(p: Prefix) -> Self {
        let keys = vec![p.to_string()];
        Self { keys }
    }

    pub fn set_prefix(&mut self, p: Prefix) -> &Self {
        self.keys[0] = p.to_string();
        self
    }

    pub fn add(mut self, s: String) -> Self {
        self.keys.push(s);
        self
    }

    pub fn get_prefix(&self) -> Prefix {
        Prefix::from_str(self.keys.get(0).unwrap()).unwrap()
    }

    pub fn get(&self, i: usize) -> String {
        let s = self.keys.get(i);
        match s {
            Some(s) => (*s).clone(),
            None => "".to_string(),
        }
    }

    pub fn get_end(&self) -> String {
        self.get(self.keys.len() - 1)
    }

    pub fn get_storage_key(&self) -> String {
        self.keys.join("_")
    }
}

impl Prefix {
    pub fn prefix(&self) -> String {
        format!("{}_", self.to_string())
    }
}

impl fmt::Display for Prefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let t = match *self {
            Self::Active => "active",
            Self::History => "history",
            _ => "",
        };
        write!(f, "{}", t)
    }
}

impl FromStr for Prefix {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let r = match s {
            "active" => Prefix::Active,
            "history" => Prefix::History,
            _ => Prefix::None,
        };
        Ok(r)
    }
}
impl FromStr for Keys {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let keys: Vec<&str> = s.split("_").collect();
        let keys = keys.iter().map(|s| s.to_string()).collect();
        Ok(Keys { keys })
    }
}
#[derive(Clone)]
pub struct Storage {
    db: Db,
}
impl Storage {
    pub fn new(store_path: PathBuf) -> anyhow::Result<Self> {
        let db = sled::open(store_path.join("accounts"))
            .map_err(|e| com::CliError::DBError(e.to_string()))?;
        Ok(Self { db })
    }

    // Active load Active account
    pub fn scan_prefix(&self, p: &Prefix) -> sled::Iter {
        let px = p.prefix();
        self.db.scan_prefix(px.as_bytes())
    }

    fn save_one(&self, ks: &Keys, data: &State) -> anyhow::Result<()> {
        let value = serde_json::to_vec(data)?;
        let key = ks.get_storage_key();
        self.db.insert(key.as_bytes(), value)?;
        Ok(())
    }

    pub fn save_to_active(&self, ks: &Keys, data: &State) -> anyhow::Result<()> {
        self.save_one(ks, data)
    }

    pub fn save_to_history(&self, ks: &mut Keys, data: &State) -> anyhow::Result<()> {
        ks.set_prefix(Prefix::History);
        self.save_one(ks, data)
    }

    pub fn save_as_history(&self, ks: &mut Keys, data: &State) -> anyhow::Result<()> {
        let value = serde_json::to_vec(data)?;
        let value = value.as_slice();
        let key = ks.get_storage_key();
        ks.set_prefix(Prefix::History);
        let history_key = ks.get_storage_key();
        self.db
            .transaction::<_, (), anyhow::Error>(|tx| {
                tx.remove(key.as_bytes())?;
                tx.insert(history_key.as_bytes(), value)?;
                Ok(())
            })
            .map_err(|e| com::CliError::DBError(e.to_string()))?;
        Ok(())
    }

    pub fn save_batch(&self, kv: Vec<(&Keys, &State)>) -> anyhow::Result<()> {
        let mut batch = Batch::default();
        for v in kv {
            let value = serde_json::to_vec(v.1)?;
            let key = v.0.get_storage_key();
            batch.insert(key.as_bytes(), value);
        }
        Ok(())
    }

    pub fn get_position_history_list(&self, address: &Address) -> sled::Iter {
        let keys = Keys::new(Prefix::History)
            .add("position".to_string())
            .add(address.to_string());
        let key = keys.get_storage_key();
        self.db.scan_prefix(key.as_bytes())
    }

    pub fn get_position_info(&self, address: &Address,position_address: &Address)-> Option<Position>{
        let keys = Keys::new(Prefix::History)
            .add("position".to_string())
            .add(address.to_string())
            .add(position_address.to_string());
        let key = keys.get_storage_key();
        let r = self.db.get(key.as_bytes());
        if let Ok(Some(v)) = r {
            let p: Position = serde_json::from_slice(v.as_ref()).unwrap();
            Some(p)
        } else {
            None
        }
    }

    pub fn get_market_history_list(&self) -> sled::Iter {
        let keys = Keys::new(Prefix::History).add("market".to_string());
        let key = keys.get_storage_key();
        self.db.scan_prefix(key.as_bytes())
    }
}
