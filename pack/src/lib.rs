pub mod gpack;

pub use gpack::{GpackPacker, GpackReader, GPACK_MAGIC, GPACK_VERSION};

use runtimepack_core::diagnostics::DiagnosticError;
use runtimepack_core::domain::ContentMode;
use std::path::Path;

pub trait ContentPacker {
    fn pack(&self, source_dir: &Path, output_file: &Path, mode: ContentMode) -> Result<u64, DiagnosticError>;
}

pub struct StandardContentPacker;

impl ContentPacker for StandardContentPacker {
    fn pack(&self, source_dir: &Path, output_file: &Path, mode: ContentMode) -> Result<u64, DiagnosticError> {
        match mode {
            ContentMode::Plain => {
                // In plain mode, files are kept loose or in asset directory
                Ok(0)
            }
            ContentMode::Packed => {
                GpackPacker::pack_directory(source_dir, output_file)
            }
            ContentMode::EncryptedStreaming => {
                // Milestone 7 will add authenticated AEAD chunk encryption
                GpackPacker::pack_directory(source_dir, output_file)
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
}
