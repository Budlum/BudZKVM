use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use tiny_keccak::{Hasher, Keccak};

pub type Hash = [u8; 32];

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Account {
    pub nonce: u64,
    pub balance: u64,
}

pub trait StateBackend {
    fn get_account(&self, id: u64) -> Option<Account>;
    fn set_account(&mut self, id: u64, account: Account);
    fn root(&self) -> Hash;
    fn begin_transaction(&mut self);
    fn commit(&mut self) -> Result<(), String>;
    fn rollback(&mut self);
}

pub struct State {
    pub accounts: HashMap<u64, Account>,
    path: String,
    backup: Option<HashMap<u64, Account>>,
}

impl State {
    pub fn load(path: &str) -> Result<Self, String> {
        let accounts = if std::path::Path::new(path).exists() {
            let data = fs::read_to_string(path)
                .map_err(|e| format!("Failed to read state file: {}", e))?;
            serde_json::from_str(&data).map_err(|e| format!("Failed to parse state JSON: {}", e))?
        } else {
            HashMap::new()
        };
        Ok(Self {
            accounts,
            path: path.to_string(),
            backup: None,
        })
    }

    pub fn save(&self) {
        self.save_atomic().expect("Failed to save state atomically");
    }

    pub fn save_atomic(&self) -> Result<(), String> {
        let data = serde_json::to_string_pretty(&self.accounts)
            .map_err(|e| format!("Failed to serialize state: {}", e))?;
        let temp_path = format!("{}.tmp", self.path);
        let mut file = fs::File::create(&temp_path)
            .map_err(|e| format!("Failed to create temp state file: {}", e))?;
        file.write_all(data.as_bytes())
            .map_err(|e| format!("Failed to write to temp state file: {}", e))?;
        file.sync_all()
            .map_err(|e| format!("Failed to sync temp state file: {}", e))?;
        drop(file);
        fs::rename(&temp_path, &self.path)
            .map_err(|e| format!("Failed to rename temp state file to final: {}", e))?;
        Ok(())
    }

    pub fn root(&self) -> Hash {
        let mut sorted_keys: Vec<_> = self.accounts.keys().collect();
        sorted_keys.sort();

        let mut hasher = Keccak::v256();
        // Domain separation
        hasher.update(b"BUDZKVM_STATE_ROOT_V1");

        for &key in sorted_keys {
            let acc = &self.accounts[&key];
            hasher.update(&key.to_le_bytes());
            hasher.update(&acc.balance.to_le_bytes());
            hasher.update(&acc.nonce.to_le_bytes());
        }

        let mut res = [0u8; 32];
        hasher.finalize(&mut res);
        res
    }
}

impl StateBackend for State {
    fn get_account(&self, id: u64) -> Option<Account> {
        self.accounts.get(&id).cloned()
    }

    fn set_account(&mut self, id: u64, account: Account) {
        self.accounts.insert(id, account);
    }

    fn root(&self) -> Hash {
        self.root()
    }

    fn begin_transaction(&mut self) {
        self.backup = Some(self.accounts.clone());
    }

    fn commit(&mut self) -> Result<(), String> {
        self.backup = None;
        self.save_atomic()
    }

    fn rollback(&mut self) {
        if let Some(backup) = self.backup.take() {
            self.accounts = backup;
        }
    }
}
