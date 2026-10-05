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

    /// Every sealed record in one blob, encrypted again under the cache key
    /// so record names stay private at rest. The platform persists this.
    pub fn seal_all(&self) -> Result<Vec<u8>, StoreError> {
        let mut names: Vec<&String> = self.records.keys().collect();
        names.sort();
        let mut frame = Vec::new();
        for name in names {
            let sealed = &self.records[name];
            frame.extend((name.len() as u32).to_be_bytes());
            frame.extend(name.as_bytes());
            frame.extend((sealed.len() as u32).to_be_bytes());
            frame.extend(sealed);
        }
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = self
            .cipher
            .encrypt(&nonce, frame.as_slice())
            .map_err(|_| StoreError::Rejected)?;
        let mut out = nonce.to_vec();
        out.extend(ciphertext);
        Ok(out)
    }

    /// Replaces every record with the blob from [`Self::seal_all`]. A blob
    /// sealed under another key is refused and the records stay.
    pub fn open_all(&mut self, blob: &[u8]) -> Result<(), StoreError> {
        if blob.len() < NONCE_LEN {
            return Err(StoreError::Rejected);
        }
        let (nonce, ciphertext) = blob.split_at(NONCE_LEN);
        let frame = self
            .cipher
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|_| StoreError::Rejected)?;
        let mut records = HashMap::new();
        let mut rest = frame.as_slice();
        while !rest.is_empty() {
            let (name, after) = take(rest)?;
            let (sealed, after) = take(after)?;
            let name = String::from_utf8(name.to_vec()).map_err(|_| StoreError::Rejected)?;
            records.insert(name, sealed.to_vec());
            rest = after;
        }
        self.records = records;
        Ok(())
    }

    /// Record names starting with `prefix`, sorted.
    pub fn names_with_prefix(&self, prefix: &str) -> Vec<String> {
        let mut names: Vec<String> = self
            .records
            .keys()
            .filter(|name| name.starts_with(prefix))
            .cloned()
            .collect();
        names.sort();
        names
    }

    pub fn remove(&mut self, name: &str) {
        self.records.remove(name);
    }

    pub fn export(&self, name: &str) -> Option<&[u8]> {
        self.records.get(name).map(Vec::as_slice)
    }

    pub fn import(&mut self, name: &str, sealed: Vec<u8>) {
        self.records.insert(name.to_string(), sealed);
    }
}

fn take(bytes: &[u8]) -> Result<(&[u8], &[u8]), StoreError> {
    let (len, after) = bytes.split_at_checked(4).ok_or(StoreError::Rejected)?;
    let len = u32::from_be_bytes(len.try_into().map_err(|_| StoreError::Rejected)?) as usize;
    after.split_at_checked(len).ok_or(StoreError::Rejected)
}
