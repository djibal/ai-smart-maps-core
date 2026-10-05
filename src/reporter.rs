//! The reporter key: 128 random bits created on the device, stored only in
//! the sealed cache, and deleted with it. It is not an account, a name, or
//! a hardware id. It is never written to a log or a contract, and its
//! `Debug` form is a fixed placeholder.

use std::fmt;

use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::OsRng;

/// The record name inside the sealed cache.
pub const RECORD: &str = "reporter-key";

const LEN: usize = 16;

#[derive(Clone, PartialEq, Eq)]
pub struct ReporterKey([u8; LEN]);

#[derive(Debug, PartialEq, Eq)]
pub enum ReporterKeyError {
    /// Not exactly 32 hexadecimal characters.
    Malformed,
}

impl ReporterKey {
    /// 16 random bytes from the operating system.
    pub fn generate() -> Self {
        let mut bytes = [0u8; LEN];
        OsRng.fill_bytes(&mut bytes);
        Self(bytes)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ReporterKeyError> {
        let bytes: [u8; LEN] = bytes.try_into().map_err(|_| ReporterKeyError::Malformed)?;
        Ok(Self(bytes))
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// 32 lowercase hexadecimal characters: the text form a report carries.
    pub fn hex(&self) -> String {
        self.0.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    /// Accepts exactly 32 hexadecimal characters of either case.
    pub fn from_hex(text: &str) -> Result<Self, ReporterKeyError> {
        if text.len() != LEN * 2 || !text.is_ascii() {
            return Err(ReporterKeyError::Malformed);
        }
        let mut bytes = [0u8; LEN];
        for (index, pair) in text.as_bytes().chunks(2).enumerate() {
            let pair = std::str::from_utf8(pair).map_err(|_| ReporterKeyError::Malformed)?;
            bytes[index] = u8::from_str_radix(pair, 16).map_err(|_| ReporterKeyError::Malformed)?;
        }
        Ok(Self(bytes))
    }
}

impl fmt::Debug for ReporterKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ReporterKey(redacted)")
    }
}
