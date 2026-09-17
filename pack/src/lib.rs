use runtimepack_core::diagnostics::DiagnosticError;
use runtimepack_core::domain::ContentMode;
use std::path::Path;

pub trait ContentPacker {
    fn pack(&self, source_dir: &Path, output_file: &Path, mode: ContentMode) -> Result<(), DiagnosticError>;
}
