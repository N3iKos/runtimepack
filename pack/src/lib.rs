pub mod crypto;
pub mod gpack;

pub use crypto::ContentCrypto;
pub use gpack::{GpackPacker, GpackReader, GPACK_MAGIC, GPACK_VERSION};

use runtimepack_core::diagnostics::DiagnosticError;
use runtimepack_core::domain::ContentMode;
use std::path::Path;

pub trait ContentPacker {
    fn pack(
        &self,
        source_dir: &Path,
        output_file: &Path,
        mode: ContentMode,
    ) -> Result<u64, DiagnosticError>;
    fn pack_with_key(
        &self,
        source_dir: &Path,
        output_file: &Path,
        mode: ContentMode,
        key: Option<&[u8; 32]>,
    ) -> Result<u64, DiagnosticError>;
    fn pack_from_index(
        &self,
        index: &runtimepack_core::ingest::GameSourceIndex,
        output_file: &Path,
        mode: ContentMode,
    ) -> Result<u64, DiagnosticError>;
    fn pack_from_index_with_key(
        &self,
        index: &runtimepack_core::ingest::GameSourceIndex,
        output_file: &Path,
        mode: ContentMode,
        key: Option<&[u8; 32]>,
    ) -> Result<u64, DiagnosticError>;
}

pub struct StandardContentPacker;

impl ContentPacker for StandardContentPacker {
    fn pack(
        &self,
        source_dir: &Path,
        output_file: &Path,
        mode: ContentMode,
    ) -> Result<u64, DiagnosticError> {
        self.pack_with_key(source_dir, output_file, mode, None)
    }

    fn pack_with_key(
        &self,
        source_dir: &Path,
        output_file: &Path,
        mode: ContentMode,
        key: Option<&[u8; 32]>,
    ) -> Result<u64, DiagnosticError> {
        match mode {
            ContentMode::Plain => Ok(0),
            ContentMode::Packed => GpackPacker::pack_directory(source_dir, output_file),
            ContentMode::EncryptedStreaming => {
                let default_key = [0x42u8; 32];
                let effective_key = key.unwrap_or(&default_key);
                GpackPacker::pack_directory_with_options(
                    source_dir,
                    output_file,
                    Some(effective_key),
                )
            }
        }
    }

    fn pack_from_index(
        &self,
        index: &runtimepack_core::ingest::GameSourceIndex,
        output_file: &Path,
        mode: ContentMode,
    ) -> Result<u64, DiagnosticError> {
        self.pack_from_index_with_key(index, output_file, mode, None)
    }

    fn pack_from_index_with_key(
        &self,
        index: &runtimepack_core::ingest::GameSourceIndex,
        output_file: &Path,
        mode: ContentMode,
        key: Option<&[u8; 32]>,
    ) -> Result<u64, DiagnosticError> {
        match mode {
            ContentMode::Plain => Ok(0),
            ContentMode::Packed => GpackPacker::pack_from_index(index, output_file),
            ContentMode::EncryptedStreaming => {
                let default_key = [0x42u8; 32];
                let effective_key = key.unwrap_or(&default_key);
                GpackPacker::pack_from_index_with_options(
                    index,
                    output_file,
                    Some(effective_key),
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use runtimepack_core::diagnostics::DiagnosticCode;
    use std::fs;

    #[test]
    fn test_path_traversal_rejection() {
        assert!(GpackPacker::validate_logical_path("../foo").is_err());
        assert!(GpackPacker::validate_logical_path("a/b/../../c").is_err());
        assert!(GpackPacker::validate_logical_path("normal/path/file.png").is_ok());
    }

    #[test]
    fn test_gpack_roundtrip() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let root = manifest_dir.parent().unwrap_or(manifest_dir);
        let src = root.join("tests").join("fixtures").join("vxace_min");

        let temp_pack = std::env::temp_dir().join("test_game.pack");
        if temp_pack.exists() {
            let _ = fs::remove_file(&temp_pack);
        }

        let written = GpackPacker::pack_directory(&src, &temp_pack).unwrap();
        assert!(written > 0);

        let mut reader = GpackReader::open(&temp_pack).unwrap();
        assert_eq!(reader.file_count(), 3);
        assert!(reader.has_file("Game.ini"));
        assert!(reader.has_file("Data/Scripts.rvdata2"));

        let data = reader.read_file("Data/Scripts.rvdata2").unwrap();
        assert_eq!(data, b"dummy rvdata2");

        // Traversal test on reader
        let err = reader.read_file("../escape").unwrap_err();
        if let DiagnosticError::Detailed { code, .. } = err {
            assert_eq!(code.0, DiagnosticCode::CNT_TRAVERSAL_REJECTED);
        }

        let _ = fs::remove_file(&temp_pack);
    }

    #[test]
    fn test_encrypted_streaming_roundtrip() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let root = manifest_dir.parent().unwrap_or(manifest_dir);
        let src = root.join("tests").join("fixtures").join("vxace_min");

        let temp_pack = std::env::temp_dir().join("test_encrypted_game.pack");
        if temp_pack.exists() {
            let _ = fs::remove_file(&temp_pack);
        }

        let key = ContentCrypto::derive_key("secure_game_passphrase", b"app_salt_123");
        let written =
            GpackPacker::pack_directory_with_options(&src, &temp_pack, Some(&key)).unwrap();
        assert!(written > 0);

        let mut reader = GpackReader::open(&temp_pack).unwrap();
        assert_eq!(reader.file_count(), 3);

        // Read with correct key
        let data = reader
            .read_file_with_key("Data/Scripts.rvdata2", Some(&key))
            .unwrap();
        assert_eq!(data, b"dummy rvdata2");

        // Read with wrong key -> MUST fail closed!
        let wrong_key = [0x99u8; 32];
        let err = reader
            .read_file_with_key("Data/Scripts.rvdata2", Some(&wrong_key))
            .unwrap_err();
        if let DiagnosticError::Detailed { code, .. } = err {
            assert_eq!(code.0, DiagnosticCode::CNT_CORRUPTED_PACK);
        } else {
            panic!("Expected DiagnosticError::Detailed");
        }

        let _ = fs::remove_file(&temp_pack);
    }

    #[test]
    fn test_derive_package_key_deterministic_and_unique() {
        let key1 = ContentCrypto::derive_package_key("com.example.lonarpg", "Lona RPG");
        let key2 = ContentCrypto::derive_package_key("com.example.lonarpg", "Lona RPG");
        let key3 = ContentCrypto::derive_package_key("com.example.othergame", "Lona RPG");
        let key4 = ContentCrypto::derive_package_key("com.example.lonarpg", "Other Game");

        assert_eq!(
            key1, key2,
            "Keys for identical package and name must be deterministic"
        );
        assert_ne!(key1, key3, "Keys for different package names must differ");
        assert_ne!(key1, key4, "Keys for different app names must differ");
        assert_ne!(key1, [0u8; 32], "Key must not be zeroes");
    }
}
