use crate::bot::state::{Event, Message, MessageSender, State, Storage};
use crate::com;
use async_trait::async_trait;
use log::error;
use sled::Db;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Clone)]
pub struct Keys {
    keys: Vec<String>,
}

impl Keys {
    pub fn new(prefix: String) -> Self {
        let keys = vec![prefix];
        Self { keys }
    }

    pub fn add(mut self, s: String) -> Self {
        self.keys.push(s);
        self
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

impl FromStr for Keys {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let keys: Vec<&str> = s.split("_").collect();
        let keys = keys.iter().map(|s| s.to_string()).collect();
        Ok(Keys { keys })
    }
}
#[derive(Clone)]
pub struct Local {
    db: Db,
}

#[async_trait]
impl Storage for Local {
    async fn save_one(&self, state: State) -> anyhow::Result<()> {
        let mut keys = Keys::new(state.to_string());
        match state {
            State::List(data) => {
                keys = keys.add(data.id.to_string());
                self.save(&keys, &State::List(data))?;
            }
            State::Market(data) => {
                keys = keys.add(data.id.to_string());
                self.save(&keys, &State::Market(data))?;
            }
            State::Account(data) => {
                keys = keys.add(data.id.to_string());
                self.save(&keys, &State::Account(data))?;
            }
            State::Position(data) => {
                keys = keys.add(data.id.to_string());
                self.save(&keys, &State::Position(data))?;
            }
            _ => {}
        }
        Ok(())
    }
    async fn load_all(&self, send: MessageSender) -> anyhow::Result<()> {
        let r = self.db.iter();
        for i in r {
            match i {
                Ok((_k, v)) => {
                    let values: State = serde_json::from_slice(v.to_vec().as_slice())
                        .map_err(|e| com::ClientError::JsonError(e.to_string()))?;
                    if let Err(e) = send.send(Message {
                        state: values,
                        event: Event::None,
                    }) {
                        error!("send msg error: {:?}", e)
                    }
                }
                Err(e) => {
                    error!("{}", e);
                }
            }
        }
        Ok(())
    }
}
impl Local {
    pub fn new(store_path: PathBuf) -> anyhow::Result<Self> {
        let db = sled::open(store_path.join("accounts"))
            .map_err(|e| com::ClientError::DBError(e.to_string()))?;
        Ok(Self { db })
    }
    // Active load Active account
    pub fn scan_prefix(&self, prefix: String) -> sled::Iter {
        self.db.scan_prefix(prefix.as_bytes())
    }

    fn save(&self, ks: &Keys, data: &State) -> anyhow::Result<()> {
        let value = serde_json::to_vec(data)?;
        self.db.insert(ks.get_storage_key().as_bytes(), value)?;
        Ok(())
    }
}
