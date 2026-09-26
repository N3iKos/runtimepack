use crate::asset_audit::{AssetKind, AssetNode, AuditCompleteness, ReferenceRequiredness};
use crate::ingest::{GameSourceIndex, IndexedFile};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolutionResult {
    Exact(AssetNode),
    /// Case mismatch between referenced path and actual file on disk.
    /// In Android VFS, case-folding is built-in (`VfsResourceInterceptor.kt` and `GpackExtractor.kt`),
    /// meaning case mismatches on disk are safely auto-aliased and resolved at runtime.
    CaseMismatch {
        expected: String,
        actual: String,
        node: AssetNode,
    },
    CaseCollision(Vec<String>),
    Missing(Vec<String>),
    Unsafe(String),
}

impl ResolutionResult {
    pub fn is_resolved(&self) -> bool {
        matches!(self, Self::Exact(_) | Self::CaseMismatch { .. })
    }

    pub fn is_exact(&self) -> bool {
        matches!(self, Self::Exact(_))
    }

    pub fn is_case_mismatch(&self) -> bool {
        matches!(self, Self::CaseMismatch { .. })
    }

    /// Indicates whether this resolution result represents a case mismatch that is safely
    /// auto-aliased by the runtime VFS (e.g. `VfsResourceInterceptor.kt` on Android).
    pub fn is_auto_aliased(&self) -> bool {
        self.is_case_mismatch()
    }

    pub fn is_case_collision(&self) -> bool {
        matches!(self, Self::CaseCollision(_))
    }

    pub fn is_missing(&self) -> bool {
        matches!(self, Self::Missing(_))
    }

    pub fn is_unsafe(&self) -> bool {
        matches!(self, Self::Unsafe(_))
    }

    pub fn node(&self) -> Option<&AssetNode> {
        match self {
            Self::Exact(node) | Self::CaseMismatch { node, .. } => Some(node),
            _ => None,
        }
    }

    pub fn actual_path(&self) -> Option<&str> {
        self.node().and_then(|node| node.logical_path.as_deref())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CandidateRule {
    Exact(String),
    WithExtensions {
        base_path: String,
        extensions: Vec<String>,
    },
    Candidates(Vec<String>),
}

impl CandidateRule {
    pub fn exact(path: impl Into<String>) -> Self {
        Self::Exact(path.into())
    }

    pub fn with_extensions(
        base_path: impl Into<String>,
        extensions: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self::WithExtensions {
            base_path: base_path.into(),
            extensions: extensions.into_iter().map(Into::into).collect(),
        }
    }

    pub fn candidates(candidates: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self::Candidates(candidates.into_iter().map(Into::into).collect())
    }

    pub fn expand_candidates(&self) -> Vec<String> {
        match self {
            Self::Exact(path) => {
                let norm = normalize_relative_path(path).unwrap_or_else(|| path.clone());
                vec![norm]
            }
            Self::Candidates(list) => {
                let mut out = Vec::with_capacity(list.len());
                for p in list {
                    let norm = normalize_relative_path(p).unwrap_or_else(|| p.clone());
                    if !out.contains(&norm) {
                        out.push(norm);
                    }
                }
                out
            }
            Self::WithExtensions {
                base_path,
                extensions,
            } => {
                let normalized_base =
                    normalize_relative_path(base_path).unwrap_or_else(|| base_path.clone());
                if extensions.is_empty() {
                    return vec![normalized_base];
                }

                // File extension must occur within the filename component after the last path separator
                let filename_start = normalized_base
                    .rfind(|c| c == '/' || c == '\\')
                    .map(|idx| idx + 1)
                    .unwrap_or(0);
                let filename = &normalized_base[filename_start..];

                if let Some(dot_offset) = filename.rfind('.') {
                    let dot_idx = filename_start + dot_offset;
                    let existing_ext = &normalized_base[dot_idx + 1..];

                    let matches_any = extensions.iter().any(|ext| {
                        let cleaned = ext.strip_prefix('.').unwrap_or(ext);
                        existing_ext.eq_ignore_ascii_case(cleaned)
                    });

                    if matches_any {
                        let stem = &normalized_base[..dot_idx];
                        let mut list = Vec::with_capacity(extensions.len());
                        list.push(normalized_base.clone());
                        for ext in extensions {
                            let cleaned = ext.strip_prefix('.').unwrap_or(ext);
                            let candidate = format!("{stem}.{cleaned}");
                            if !candidate.eq_ignore_ascii_case(&normalized_base) {
                                list.push(candidate);
                            }
                        }
                        return list;
                    }
                }

                let mut list = Vec::with_capacity(extensions.len());
                for ext in extensions {
                    let cleaned = ext.strip_prefix('.').unwrap_or(ext);
                    if cleaned.is_empty() {
                        list.push(normalized_base.clone());
                    } else {
                        list.push(format!("{normalized_base}.{cleaned}"));
                    }
                }
                list
            }
        }
    }
}

impl From<&str> for CandidateRule {
    fn from(s: &str) -> Self {
        Self::Exact(s.to_string())
    }
}

impl From<String> for CandidateRule {
    fn from(s: String) -> Self {
        Self::Exact(s)
    }
}

impl From<Vec<String>> for CandidateRule {
    fn from(candidates: Vec<String>) -> Self {
        Self::Candidates(candidates)
    }
}

pub struct AssetResolver<'a> {
    source: &'a GameSourceIndex,
    exact_map: HashMap<String, &'a IndexedFile>,
    case_folded_map: HashMap<String, Vec<&'a IndexedFile>>,
}

impl<'a> AssetResolver<'a> {
    pub fn new(source: &'a GameSourceIndex) -> Self {
        let mut exact_map = HashMap::with_capacity(source.files.len());
        let mut case_folded_map: HashMap<String, Vec<&'a IndexedFile>> =
            HashMap::with_capacity(source.files.len());

        for file in source.files.values() {
            exact_map.insert(file.logical_path.clone(), file);
            let folded = case_fold(&file.logical_path);
            case_folded_map.entry(folded).or_default().push(file);
        }

        for files in case_folded_map.values_mut() {
            files.sort_by(|a, b| a.logical_path.cmp(&b.logical_path));
            files.dedup_by(|a, b| a.logical_path == b.logical_path);
        }

        Self {
            source,
            exact_map,
            case_folded_map,
        }
    }

    pub fn source(&self) -> &'a GameSourceIndex {
        self.source
    }

    pub fn get_exact(&self, path: &str) -> Option<&'a IndexedFile> {
        let normalized = normalize_candidate(path);
        self.exact_map.get(&normalized).copied()
    }

    pub fn contains_exact(&self, path: &str) -> bool {
        let normalized = normalize_candidate(path);
        self.exact_map.contains_key(&normalized)
    }

    pub fn resolve(&self, rule: &CandidateRule, kind: AssetKind) -> ResolutionResult {
        let candidates = rule.expand_candidates();
        if candidates.is_empty() {
            return ResolutionResult::Missing(Vec::new());
        }

        for candidate in &candidates {
            if is_unsafe_path(candidate) {
                return ResolutionResult::Unsafe(candidate.clone());
            }
        }

        for candidate in &candidates {
            let normalized = normalize_candidate(candidate);
            let folded = case_fold(&normalized);

            if let Some(matching_files) = self.case_folded_map.get(&folded) {
                if matching_files.len() > 1 {
                    let colliding: Vec<String> = matching_files
                        .iter()
                        .map(|f| f.logical_path.clone())
                        .collect();
                    return ResolutionResult::CaseCollision(colliding);
                }

                let actual_file = matching_files[0];
                let actual_path = &actual_file.logical_path;

                let node = AssetNode {
                    id: format!("{}:{actual_path}", kind.as_str()),
                    kind,
                    logical_path: Some(actual_path.clone()),
                };

                if actual_path == &normalized {
                    return ResolutionResult::Exact(node);
                } else {
                    return ResolutionResult::CaseMismatch {
                        expected: candidate.clone(),
                        actual: actual_path.clone(),
                        node,
                    };
                }
            }
        }

        ResolutionResult::Missing(candidates)
    }

    pub fn resolve_path(&self, path: &str, kind: AssetKind) -> ResolutionResult {
        self.resolve(&CandidateRule::exact(path), kind)
    }

    pub fn resolve_with_extensions(
        &self,
        base_path: &str,
        extensions: &[impl AsRef<str>],
        kind: AssetKind,
    ) -> ResolutionResult {
        let exts: Vec<String> = extensions.iter().map(|e| e.as_ref().to_string()).collect();
        self.resolve(&CandidateRule::with_extensions(base_path, exts), kind)
    }
}

fn normalize_path_slashes(path: &str) -> String {
    path.replace('\\', "/")
}

fn case_fold(path: &str) -> String {
    normalize_candidate(path).to_lowercase()
}

fn normalize_candidate(path: &str) -> String {
    if let Some(normalized) = normalize_relative_path(path) {
        normalized
    } else {
        let replaced = normalize_path_slashes(path);
        let mut components = Vec::new();
        for part in replaced.split('/') {
            if part.is_empty() || part == "." {
                continue;
            }
            components.push(part);
        }
        components.join("/")
    }
}

fn has_drive_letter(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

pub fn normalize_relative_path(path: &str) -> Option<String> {
    if path.contains('\0') {
        return None;
    }

    let replaced = normalize_path_slashes(path);

    // Disguised path check: strip leading "./" prefixes
    let mut stripped = replaced.as_str();
    while let Some(rest) = stripped.strip_prefix("./") {
        stripped = rest;
    }

    // Absolute Unix or UNC
    if stripped.starts_with('/') || has_drive_letter(stripped) {
        return None;
    }

    let mut components = Vec::new();
    for part in stripped.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            if components.pop().is_none() {
                // Attempting to traverse above root
                return None;
            }
        } else {
            if has_drive_letter(part) {
                return None;
            }
            components.push(part);
        }
    }

    Some(components.join("/"))
}

fn is_unsafe_path(path: &str) -> bool {
    normalize_relative_path(path).is_none()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CoverageLevel {
    Covered,
    Partial,
    Unsupported,
    Dynamic,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceClassCoverage {
    pub name: String,
    pub level: CoverageLevel,
    pub requiredness: ReferenceRequiredness,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditCoverage {
    classes: BTreeMap<String, ReferenceClassCoverage>,
}

impl AuditCoverage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn declare(
        &mut self,
        name: impl Into<String>,
        level: CoverageLevel,
        requiredness: ReferenceRequiredness,
    ) -> &mut Self {
        let name = name.into();
        self.classes.insert(
            name.clone(),
            ReferenceClassCoverage {
                name,
                level,
                requiredness,
            },
        );
        self
    }

    pub fn add_covered(&mut self, name: impl Into<String>) -> &mut Self {
        self.declare(
            name,
            CoverageLevel::Covered,
            ReferenceRequiredness::Required,
        )
    }

    pub fn add_covered_with(
        &mut self,
        name: impl Into<String>,
        requiredness: ReferenceRequiredness,
    ) -> &mut Self {
        self.declare(name, CoverageLevel::Covered, requiredness)
    }

    pub fn add_partial(
        &mut self,
        name: impl Into<String>,
        requiredness: ReferenceRequiredness,
    ) -> &mut Self {
        self.declare(name, CoverageLevel::Partial, requiredness)
    }

    pub fn add_unsupported(
        &mut self,
        name: impl Into<String>,
        requiredness: ReferenceRequiredness,
    ) -> &mut Self {
        self.declare(name, CoverageLevel::Unsupported, requiredness)
    }

    pub fn add_dynamic(
        &mut self,
        name: impl Into<String>,
        requiredness: ReferenceRequiredness,
    ) -> &mut Self {
        self.declare(name, CoverageLevel::Dynamic, requiredness)
    }

    pub fn is_required_scope_complete(&self) -> bool {
        !self.classes.values().any(|c| {
            c.requiredness == ReferenceRequiredness::Required
                && matches!(
                    c.level,
                    CoverageLevel::Unsupported | CoverageLevel::Dynamic | CoverageLevel::Partial
                )
        })
    }

    pub fn covered_classes(&self) -> Vec<String> {
        self.classes
            .values()
            .filter(|c| c.level == CoverageLevel::Covered)
            .map(|c| c.name.clone())
            .collect()
    }

    pub fn partial_classes(&self) -> Vec<String> {
        self.classes
            .values()
            .filter(|c| c.level == CoverageLevel::Partial)
            .map(|c| c.name.clone())
            .collect()
    }

    pub fn unsupported_classes(&self) -> Vec<String> {
        self.classes
            .values()
            .filter(|c| c.level == CoverageLevel::Unsupported)
            .map(|c| c.name.clone())
            .collect()
    }

    pub fn dynamic_classes(&self) -> Vec<String> {
        self.classes
            .values()
            .filter(|c| c.level == CoverageLevel::Dynamic)
            .map(|c| c.name.clone())
            .collect()
    }

    pub fn has_unsupported_or_dynamic(&self) -> bool {
        self.classes
            .values()
            .any(|c| matches!(c.level, CoverageLevel::Unsupported | CoverageLevel::Dynamic))
    }

    pub fn from_completeness(completeness: &AuditCompleteness) -> Self {
        let mut cov = Self::new();
        for class_name in &completeness.covered_reference_classes {
            cov.add_covered(class_name);
        }
        for class_name in &completeness.uncovered_reference_classes {
            cov.add_dynamic(class_name, ReferenceRequiredness::Required);
        }
        if !completeness.required_scope_complete
            && completeness.uncovered_reference_classes.is_empty()
        {
            cov.add_unsupported("uncovered_scope", ReferenceRequiredness::Required);
        }
        cov
    }

    pub fn to_completeness(&self) -> AuditCompleteness {
        let mut covered = self.covered_classes();
        covered.sort();
        covered.dedup();

        let mut uncovered = Vec::new();
        uncovered.extend(self.partial_classes());
        uncovered.extend(self.unsupported_classes());
        uncovered.extend(self.dynamic_classes());
        uncovered.sort();
        uncovered.dedup();

        AuditCompleteness {
            required_scope_complete: self.is_required_scope_complete(),
            covered_reference_classes: covered,
            uncovered_reference_classes: uncovered,
        }
    }
}

impl From<&AuditCoverage> for AuditCompleteness {
    fn from(coverage: &AuditCoverage) -> Self {
        coverage.to_completeness()
    }
}

impl From<AuditCoverage> for AuditCompleteness {
    fn from(coverage: AuditCoverage) -> Self {
        coverage.to_completeness()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest::IndexedFile;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn make_index(paths: &[&str]) -> GameSourceIndex {
        let mut files = HashMap::new();
        for &p in paths {
            let normalized = p.replace('\\', "/");
            files.insert(
                normalized.clone(),
                IndexedFile {
                    logical_path: normalized.clone(),
                    relative_path: PathBuf::from(&normalized),
                    size_bytes: 100,
                    source_root: None,
                },
            );
        }
        GameSourceIndex {
            root_path: PathBuf::from("mock_root"),
            files,
            total_size_bytes: 100 * paths.len() as u64,
            source_hash: "mock_hash".into(),
            rtp_root: None,
        }
    }

    #[test]
    fn test_exact_path_resolution() {
        let index = make_index(&["Graphics/Titles/Title.png"]);
        let resolver = AssetResolver::new(&index);
        let result = resolver.resolve_path("Graphics/Titles/Title.png", AssetKind::Image);
        match result {
            ResolutionResult::Exact(node) => {
                assert_eq!(node.logical_path, Some("Graphics/Titles/Title.png".into()));
                assert_eq!(node.kind, AssetKind::Image);
                assert_eq!(node.id, "image:Graphics/Titles/Title.png");
            }
            other => panic!("expected Exact, got {other:?}"),
        }
    }

    #[test]
    fn test_case_mismatch_resolution() {
        let index = make_index(&["Graphics/Titles/Title.png"]);
        let resolver = AssetResolver::new(&index);
        let result = resolver.resolve_path("graphics/titles/title.png", AssetKind::Image);
        assert!(result.is_auto_aliased());
        match result {
            ResolutionResult::CaseMismatch {
                expected,
                actual,
                node,
            } => {
                assert_eq!(expected, "graphics/titles/title.png");
                assert_eq!(actual, "Graphics/Titles/Title.png");
                assert_eq!(node.logical_path, Some("Graphics/Titles/Title.png".into()));
                assert_eq!(node.kind, AssetKind::Image);
                assert_eq!(node.id, "image:Graphics/Titles/Title.png");
            }
            other => panic!("expected CaseMismatch, got {other:?}"),
        }
    }

    #[test]
    fn test_case_collision_resolution() {
        let index = make_index(&["Audio/BGM/Battle.ogg", "audio/bgm/battle.ogg"]);
        let resolver = AssetResolver::new(&index);
        let result = resolver.resolve_path("Audio/BGM/Battle.ogg", AssetKind::Audio);
        match result {
            ResolutionResult::CaseCollision(paths) => {
                assert_eq!(paths, vec!["Audio/BGM/Battle.ogg", "audio/bgm/battle.ogg"]);
            }
            other => panic!("expected CaseCollision, got {other:?}"),
        }
    }

    #[test]
    fn test_traversal_unsafe() {
        let index = make_index(&["Graphics/Titles/Title.png"]);
        let resolver = AssetResolver::new(&index);

        for unsafe_path in [
            "../secret.txt",
            "Graphics/../../secret.txt",
            "Data/../../secret.txt",
            "..\\windows\\system32",
            "./..",
            "./../secret",
            ".\\..",
        ] {
            let result = resolver.resolve_path(unsafe_path, AssetKind::Image);
            match result {
                ResolutionResult::Unsafe(path) => {
                    assert_eq!(path, unsafe_path);
                }
                other => panic!("expected Unsafe for {unsafe_path}, got {other:?}"),
            }
        }
    }

    #[test]
    fn test_absolute_path_unsafe() {
        let index = make_index(&[]);
        let resolver = AssetResolver::new(&index);

        for abs_path in [
            "/etc/passwd",
            "\\Windows\\System32",
            "C:\\boot.ini",
            "D:/game/data.json",
            ".//etc/passwd",
            "./C:/Windows",
            ".//./etc/shadow",
        ] {
            let result = resolver.resolve_path(abs_path, AssetKind::Image);
            match result {
                ResolutionResult::Unsafe(path) => {
                    assert_eq!(path, abs_path);
                }
                other => panic!("expected Unsafe for {abs_path}, got {other:?}"),
            }
        }
    }

    #[test]
    fn test_nul_byte_unsafe() {
        let index = make_index(&[]);
        let resolver = AssetResolver::new(&index);
        let result = resolver.resolve_path("Graphics/Title\0.png", AssetKind::Image);
        match result {
            ResolutionResult::Unsafe(path) => {
                assert_eq!(path, "Graphics/Title\0.png");
            }
            other => panic!("expected Unsafe for nul byte, got {other:?}"),
        }
    }

    #[test]
    fn test_unicode_resolution() {
        let index = make_index(&["Graphics/Characters/勇者_Épée.png"]);
        let resolver = AssetResolver::new(&index);

        // Exact unicode
        let exact = resolver.resolve_path("Graphics/Characters/勇者_Épée.png", AssetKind::Image);
        match exact {
            ResolutionResult::Exact(node) => {
                assert_eq!(
                    node.logical_path,
                    Some("Graphics/Characters/勇者_Épée.png".into())
                );
            }
            other => panic!("expected Exact for unicode, got {other:?}"),
        }

        // Case mismatch unicode
        let mismatch = resolver.resolve_path("graphics/characters/勇者_épée.png", AssetKind::Image);
        match mismatch {
            ResolutionResult::CaseMismatch {
                expected,
                actual,
                node,
            } => {
                assert_eq!(expected, "graphics/characters/勇者_épée.png");
                assert_eq!(actual, "Graphics/Characters/勇者_Épée.png");
                assert_eq!(
                    node.logical_path,
                    Some("Graphics/Characters/勇者_Épée.png".into())
                );
            }
            other => panic!("expected CaseMismatch for unicode, got {other:?}"),
        }
    }

    #[test]
    fn test_extension_candidates() {
        let index = make_index(&["Audio/SE/Jump.wav"]);
        let resolver = AssetResolver::new(&index);
        let rule = CandidateRule::with_extensions("Audio/SE/Jump", ["ogg", "mp3", "wav"]);
        let result = resolver.resolve(&rule, AssetKind::Audio);
        match result {
            ResolutionResult::Exact(node) => {
                assert_eq!(node.logical_path, Some("Audio/SE/Jump.wav".into()));
            }
            other => panic!("expected Exact, got {other:?}"),
        }
    }

    #[test]
    fn test_deterministic_candidate_order() {
        let index = make_index(&["Audio/BGM/Field.ogg", "Audio/BGM/Field.mp3"]);
        let resolver = AssetResolver::new(&index);

        let rule_ogg_first = CandidateRule::with_extensions("Audio/BGM/Field", ["ogg", "mp3"]);
        let res_ogg = resolver.resolve(&rule_ogg_first, AssetKind::Audio);
        match res_ogg {
            ResolutionResult::Exact(node) => {
                assert_eq!(node.logical_path, Some("Audio/BGM/Field.ogg".into()));
            }
            other => panic!("expected Exact Field.ogg, got {other:?}"),
        }

        let rule_mp3_first = CandidateRule::with_extensions("Audio/BGM/Field", ["mp3", "ogg"]);
        let res_mp3 = resolver.resolve(&rule_mp3_first, AssetKind::Audio);
        match res_mp3 {
            ResolutionResult::Exact(node) => {
                assert_eq!(node.logical_path, Some("Audio/BGM/Field.mp3".into()));
            }
            other => panic!("expected Exact Field.mp3, got {other:?}"),
        }
    }

    #[test]
    fn test_missing_candidates() {
        let index = make_index(&[]);
        let resolver = AssetResolver::new(&index);
        let rule = CandidateRule::with_extensions("Audio/BGM/Missing", ["ogg", "mp3"]);
        let result = resolver.resolve(&rule, AssetKind::Audio);
        match result {
            ResolutionResult::Missing(attempted) => {
                assert_eq!(
                    attempted,
                    vec!["Audio/BGM/Missing.ogg", "Audio/BGM/Missing.mp3"]
                );
            }
            other => panic!("expected Missing, got {other:?}"),
        }
    }

    #[test]
    fn test_audit_coverage_complete_when_all_covered() {
        let mut cov = AuditCoverage::new();
        cov.add_covered("maps");
        cov.add_covered("database");

        assert!(cov.is_required_scope_complete());
        let completeness = cov.to_completeness();
        assert!(completeness.required_scope_complete);
        assert_eq!(
            completeness.covered_reference_classes,
            vec!["database", "maps"]
        );
        assert!(completeness.uncovered_reference_classes.is_empty());
    }

    #[test]
    fn test_audit_coverage_unsupported_required_forces_indeterminate() {
        let mut cov = AuditCoverage::new();
        cov.add_covered("maps");
        cov.add_unsupported("scripts", ReferenceRequiredness::Required);

        assert!(!cov.is_required_scope_complete());
        let completeness = cov.to_completeness();
        assert!(!completeness.required_scope_complete);
        assert_eq!(completeness.covered_reference_classes, vec!["maps"]);
        assert_eq!(completeness.uncovered_reference_classes, vec!["scripts"]);
    }

    #[test]
    fn test_audit_coverage_unsupported_optional_does_not_force_indeterminate() {
        let mut cov = AuditCoverage::new();
        cov.add_covered("maps");
        cov.add_unsupported("movies", ReferenceRequiredness::Optional);

        assert!(cov.is_required_scope_complete());
        let completeness = cov.to_completeness();
        assert!(completeness.required_scope_complete);
        assert_eq!(completeness.covered_reference_classes, vec!["maps"]);
        assert_eq!(completeness.uncovered_reference_classes, vec!["movies"]);
    }

    #[test]
    fn test_audit_coverage_dynamic_required_forces_indeterminate() {
        let mut cov = AuditCoverage::new();
        cov.add_covered("maps");
        cov.add_dynamic("eval_code", ReferenceRequiredness::Required);

        assert!(!cov.is_required_scope_complete());
        let completeness = cov.to_completeness();
        assert!(!completeness.required_scope_complete);
        assert_eq!(completeness.covered_reference_classes, vec!["maps"]);
        assert_eq!(completeness.uncovered_reference_classes, vec!["eval_code"]);
    }

    #[test]
    fn test_audit_coverage_partial_appears_in_uncovered() {
        let mut cov = AuditCoverage::new();
        cov.add_covered("maps");
        cov.add_partial("events", ReferenceRequiredness::Optional);

        let completeness = cov.to_completeness();
        assert_eq!(completeness.covered_reference_classes, vec!["maps"]);
        assert_eq!(completeness.uncovered_reference_classes, vec!["events"]);
    }

    #[test]
    fn test_get_exact_and_contains_exact() {
        let index = make_index(&["Data/System.json", "Graphics/Titles/Title.png"]);
        let resolver = AssetResolver::new(&index);

        assert!(resolver.contains_exact("Data/System.json"));
        assert!(resolver.contains_exact("Data\\System.json"));
        assert!(resolver.contains_exact("./Data/System.json"));
        assert!(!resolver.contains_exact("data/system.json")); // case mismatch is not exact

        let exact_file = resolver.get_exact("Data/System.json").unwrap();
        assert_eq!(exact_file.logical_path, "Data/System.json");
        assert!(resolver.get_exact("data/system.json").is_none());
    }

    #[test]
    fn test_middle_traversal_unsafe() {
        let index = make_index(&["Audio/BGM/Battle.ogg"]);
        let resolver = AssetResolver::new(&index);

        let result = resolver.resolve_path("Audio/../Audio/BGM/Battle.ogg", AssetKind::Audio);
        match result {
            ResolutionResult::Exact(node) => {
                assert_eq!(node.logical_path, Some("Audio/BGM/Battle.ogg".into()));
            }
            other => panic!("expected Exact Audio/BGM/Battle.ogg, got {other:?}"),
        }
    }

    #[test]
    fn test_normalize_relative_path() {
        assert_eq!(
            normalize_relative_path("data/image/../fgimage/char.png"),
            Some("data/fgimage/char.png".to_string())
        );
        assert_eq!(
            normalize_relative_path("data/image/../fgimage/default/ボタン/出口ボタン.png"),
            Some("data/fgimage/default/ボタン/出口ボタン.png".to_string())
        );
        assert_eq!(
            normalize_relative_path("data/sound/../sound/G/Gpis_lowC.mp3"),
            Some("data/sound/G/Gpis_lowC.mp3".to_string())
        );
        assert_eq!(
            normalize_relative_path("a/b/c/../../d"),
            Some("a/d".to_string())
        );
        assert_eq!(
            normalize_relative_path("Audio/../Audio/BGM/Battle.ogg"),
            Some("Audio/BGM/Battle.ogg".to_string())
        );
        assert_eq!(
            normalize_relative_path("Data/../secret.txt"),
            Some("secret.txt".to_string())
        );
        assert_eq!(normalize_relative_path("../secret.txt"), None);
        assert_eq!(normalize_relative_path("Data/../../secret.txt"), None);
        assert_eq!(normalize_relative_path("/etc/passwd"), None);
        assert_eq!(normalize_relative_path("C:\\Windows"), None);
        assert_eq!(normalize_relative_path("Graphics/Title\0.png"), None);
    }

    #[test]
    fn test_internal_traversal_safe_resolution() {
        let index = make_index(&["secret.txt"]);
        let resolver = AssetResolver::new(&index);

        let result = resolver.resolve_path("Data/../secret.txt", AssetKind::Image);
        match result {
            ResolutionResult::Exact(node) => {
                assert_eq!(node.logical_path, Some("secret.txt".into()));
            }
            other => panic!("expected Exact secret.txt, got {other:?}"),
        }
    }

    #[test]
    fn test_extension_candidates_preserves_stem_when_extension_already_present() {
        let rule = CandidateRule::with_extensions("Graphics/Pictures/pic.png", ["png", "rpgmvp"]);
        let candidates = rule.expand_candidates();
        assert_eq!(
            candidates,
            vec!["Graphics/Pictures/pic.png", "Graphics/Pictures/pic.rpgmvp"]
        );
    }

    #[test]
    fn test_custom_candidates() {
        let index = make_index(&["Alt/Title.png"]);
        let resolver = AssetResolver::new(&index);
        let rule = CandidateRule::candidates([
            "Primary/Title.png".to_string(),
            "Alt/Title.png".to_string(),
        ]);
        let result = resolver.resolve(&rule, AssetKind::Image);
        match result {
            ResolutionResult::Exact(node) => {
                assert_eq!(node.logical_path, Some("Alt/Title.png".into()));
            }
            other => panic!("expected Exact Alt/Title.png, got {other:?}"),
        }
    }

    #[test]
    fn test_audit_coverage_has_unsupported_or_dynamic() {
        let mut cov = AuditCoverage::new();
        assert!(!cov.has_unsupported_or_dynamic());

        cov.add_covered("maps");
        assert!(!cov.has_unsupported_or_dynamic());

        cov.add_partial("events", ReferenceRequiredness::Optional);
        assert!(!cov.has_unsupported_or_dynamic());

        cov.add_unsupported("scripts", ReferenceRequiredness::Optional);
        assert!(cov.has_unsupported_or_dynamic());
    }

    #[test]
    fn test_redundant_slashes_normalized() {
        let index = make_index(&["Graphics/Titles/Title.png"]);
        let resolver = AssetResolver::new(&index);

        let result = resolver.resolve_path("Graphics//Titles///Title.png", AssetKind::Image);
        match result {
            ResolutionResult::Exact(node) => {
                assert_eq!(node.logical_path, Some("Graphics/Titles/Title.png".into()));
            }
            other => panic!("expected Exact for redundant slashes, got {other:?}"),
        }

        assert!(resolver.contains_exact("Graphics//Titles///Title.png"));
    }

    #[test]
    fn test_candidate_extension_precedence_case_mismatch_beats_subsequent_exact() {
        // Disk has Audio/BGM/Battle.ogg (uppercase Battle) and Audio/BGM/battle.mp3 (exact lowercase battle)
        let index = make_index(&["Audio/BGM/Battle.ogg", "Audio/BGM/battle.mp3"]);
        let resolver = AssetResolver::new(&index);

        // Requested base is Audio/BGM/battle with candidates [.ogg, .mp3].
        // Because .ogg is probed first and exists with case mismatch, it MUST take precedence over .mp3
        let rule = CandidateRule::with_extensions("Audio/BGM/battle", ["ogg", "mp3"]);
        let result = resolver.resolve(&rule, AssetKind::Audio);

        match result {
            ResolutionResult::CaseMismatch {
                expected,
                actual,
                node,
            } => {
                assert_eq!(expected, "Audio/BGM/battle.ogg");
                assert_eq!(actual, "Audio/BGM/Battle.ogg");
                assert_eq!(node.logical_path, Some("Audio/BGM/Battle.ogg".into()));
            }
            other => panic!("expected CaseMismatch on .ogg to take precedence, got {other:?}"),
        }
    }

    #[test]
    fn test_extension_candidates_with_dot_in_directory() {
        let rule =
            CandidateRule::with_extensions("v1.0/Graphics/Pictures/pic.png", ["png", "rpgmvp"]);
        assert_eq!(
            rule.expand_candidates(),
            vec![
                "v1.0/Graphics/Pictures/pic.png",
                "v1.0/Graphics/Pictures/pic.rpgmvp"
            ]
        );

        let rule_no_ext =
            CandidateRule::with_extensions("v1.0/Graphics/Pictures/pic", ["png", "rpgmvp"]);
        assert_eq!(
            rule_no_ext.expand_candidates(),
            vec![
                "v1.0/Graphics/Pictures/pic.png",
                "v1.0/Graphics/Pictures/pic.rpgmvp"
            ]
        );
    }
}
