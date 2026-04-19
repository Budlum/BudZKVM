use std::collections::HashMap;

pub type Hash = [u8; 32];

pub struct Account {
    pub nonce: u64,
    pub balance: u64,
    pub storage_root: Hash,
    pub code_hash: Hash,
}

impl Account {
    pub fn hash(&self) -> Hash {
        let mut res = [0u8; 32];
        res[0..8].copy_from_slice(&self.nonce.to_le_bytes());
        res[8..16].copy_from_slice(&self.balance.to_le_bytes());
        res
    }
}

pub struct State {
    pub accounts: HashMap<[u8; 20], Account>,
}

impl State {
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
        }
    }

    pub fn get_account(&self, address: &[u8; 20]) -> Option<&Account> {
        self.accounts.get(address)
    }

    pub fn update_account(&mut self, address: [u8; 20], account: Account) {
        self.accounts.insert(address, account);
    }

    pub fn root(&self) -> Hash {
        let mut hashes: Vec<Hash> = self.accounts.values().map(|a| a.hash()).collect();
        hashes.sort();
        
        if hashes.is_empty() {
            return [0u8; 32];
        }

        let mut root = hashes[0];
        for i in 1..hashes.len() {
            root = self.combine(root, hashes[i]);
        }
        root
    }

    fn combine(&self, a: Hash, b: Hash) -> Hash {
        let mut res = [0u8; 32];
        for i in 0..32 {
            res[i] = a[i] ^ b[i];
        }
        res
    }
}
