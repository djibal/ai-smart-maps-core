//! Local data at rest. The key is 32 bytes supplied by the platform secure
//! store. This module does not take a password and does not derive a key.

use std::collections::HashMap;

use aes_gcm::aead::{Aead, AeadCore, OsRng};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};

const NONCE_LEN: usize = 12;

#[derive(Debug, PartialEq, Eq)]
pub enum StoreError {
    Rejected,
    NotDue,
}

pub struct LocalStore {
    cipher: Aes256Gcm,
    records: HashMap<String, Vec<u8>>,
}

impl LocalStore {
    pub fn open(key: &[u8]) -> Result<Self, StoreError> {
        let key: [u8; 32] = key.try_into().map_err(|_| StoreError::Rejected)?;
        Ok(Self {
            cipher: Aes256Gcm::new((&key).into()),
            records: HashMap::new(),
        })
    }

    pub fn put(&mut self, name: &str, plaintext: &[u8]) -> Result<(), StoreError> {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = self
            .cipher
            .encrypt(&nonce, plaintext)
            .map_err(|_| StoreError::Rejected)?;
        let mut stored = nonce.to_vec();
        stored.extend(ciphertext);
        self.records.insert(name.to_string(), stored);
        Ok(())
    }

    pub fn get(&self, name: &str) -> Result<Vec<u8>, StoreError> {
        let stored = self.records.get(name).ok_or(StoreError::Rejected)?;
        if stored.len() < NONCE_LEN {
            return Err(StoreError::Rejected);
        }
        let (nonce, ciphertext) = stored.split_at(NONCE_LEN);
        self.cipher
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|_| StoreError::Rejected)
    }

    /// Deletes every sealed record. The key stays until [`Self::rotate`].
    pub fn clear(&mut self) {
        self.records.clear();
    }

    /// Replaces the key and deletes every sealed record once `age_days` is
    /// at least 90. The caller supplies that age and the new 32-byte key.
    /// A shorter age leaves the records and the current key in place.
    pub fn rotate(&mut self, new_key: &[u8], age_days: u32) -> Result<(), StoreError> {
        if age_days < 90 {
            return Err(StoreError::NotDue);
        }
        let key: [u8; 32] = new_key.try_into().map_err(|_| StoreError::Rejected)?;
        self.records.clear();
        self.cipher = Aes256Gcm::new((&key).into());
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn export(&self, name: &str) -> Option<&[u8]> {
        self.records.get(name).map(Vec::as_slice)
    }

    pub fn import(&mut self, name: &str, sealed: Vec<u8>) {
        self.records.insert(name.to_string(), sealed);
    }
}
