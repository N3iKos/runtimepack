use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use runtimepack_core::diagnostics::{DiagnosticCode, DiagnosticError};
use sha2::{Digest, Sha256};

pub const DEFAULT_CHUNK_SIZE: usize = 65536; // 64 KB

pub struct ContentCrypto;

impl ContentCrypto {
    pub fn derive_key(passphrase: &str, salt: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(b"RuntimePack-KeyDerivation-v1");
        hasher.update(salt);
        hasher.update(passphrase.as_bytes());
        hasher.finalize().into()
    }

    pub fn make_nonce(chunk_index: u64) -> Nonce {
        let mut bytes = [0u8; 12];
        bytes[4..12].copy_from_slice(&chunk_index.to_be_bytes());
        *Nonce::from_slice(&bytes)
    }

    pub fn encrypt_chunk(
        key: &[u8; 32],
        chunk_index: u64,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, DiagnosticError> {
        let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
        let nonce = Self::make_nonce(chunk_index);

        cipher
            .encrypt(&nonce, plaintext)
            .map_err(|_| {
                DiagnosticError::from_code(
                    DiagnosticCode::CNT_CORRUPTED_PACK,
                    format!("Failed to encrypt chunk #{}", chunk_index),
                )
            })
    }

    pub fn decrypt_chunk(
        key: &[u8; 32],
        chunk_index: u64,
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, DiagnosticError> {
        let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
        let nonce = Self::make_nonce(chunk_index);

        cipher
            .decrypt(&nonce, ciphertext)
            .map_err(|_| {
                DiagnosticError::from_code(
                    DiagnosticCode::CNT_CORRUPTED_PACK,
                    format!(
                        "Integrity verification failed for chunk #{}. Wrong key or data corrupted.",
                        chunk_index
                    ),
                )
            })
    }
}
