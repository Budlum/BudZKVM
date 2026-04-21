use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use std::fs;

pub type Hash = [u8; 32];

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Account {
    pub nonce: u64,
    pub balance: u64,
}

pub struct State {
    pub accounts: HashMap<u64, Account>,
    path: String,
}

impl State {
    pub fn load(path: &str) -> Self {
        let accounts = if let Ok(data) = fs::read_to_string(path) {
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            HashMap::new()
        };
        Self { accounts, path: path.to_string() }
    }

    pub fn save(&self) {
        let data = serde_json::to_string_pretty(&self.accounts).expect("Failed to serialize state");
        fs::write(&self.path, data).expect("Failed to save state");
    }

    pub fn root(&self) -> Hash {
        let mut res = [0u8; 32];
        let mut sorted_keys: Vec<_> = self.accounts.keys().collect();
        sorted_keys.sort();
        
        for key in sorted_keys {
            let acc = &self.accounts[key];
            for i in 0..8 {
                res[i] ^= ((key >> (i * 8)) & 0xFF) as u8;
                res[i+8] ^= ((acc.balance >> (i * 8)) & 0xFF) as u8;
                res[i+16] ^= ((acc.nonce >> (i * 8)) & 0xFF) as u8;
            }
        }
        res
    }
}
