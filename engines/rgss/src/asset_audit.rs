use runtimepack_core::asset_audit::{
    audit_finding_classification, AssetAuditOptions, AssetAuditReport, AssetAuditor, AssetKind,
    AssetNode, AssetResolver, AuditCoverage, AuditFinding, AuditMode, AuditOutcome,
    AuditStatistics, CandidateRule, DependencyGraph, EvidenceStage, EvidenceTrail,
    FindingDisposition, FindingSeverity, ReferenceEdge, ReferenceRequiredness, ReferenceResolution,
    ResolutionResult, RuntimeValidationState,
};
use runtimepack_core::detection::EngineDescriptor;
use runtimepack_core::diagnostics::DiagnosticCode;
use runtimepack_core::domain::EngineId;
use runtimepack_core::ingest::GameSourceIndex;
use std::sync::atomic::{AtomicU64, Ordering};

static FINDING_COUNTER: AtomicU64 = AtomicU64::new(1);

fn next_finding_id(code: &str) -> String {
    let count = FINDING_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{code}-{count}")
}

fn make_finding(
    code: &'static str,
    requiredness: ReferenceRequiredness,
    message: String,
    source_node_ids: Vec<String>,
) -> AuditFinding {
    let (severity, disposition) = audit_finding_classification(code, requiredness, false)
        .unwrap_or((FindingSeverity::Warning, FindingDisposition::Warning));
    AuditFinding {
        id: next_finding_id(code),
        code: code.to_string(),
        severity,
        disposition,
        requiredness,
        message,
        source_node_ids,
        orphan: false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RgssGeneration {
    Xp,
    Vx,
    VxAce,
}

impl RgssGeneration {
    pub fn for_engine(engine: EngineId) -> Option<Self> {
        match engine {
            EngineId::RpgMakerXp => Some(Self::Xp),
            EngineId::RpgMakerVx => Some(Self::Vx),
            EngineId::RpgMakerVxAce => Some(Self::VxAce),
            _ => None,
        }
    }

    pub fn data_extension(&self) -> &'static str {
        match self {
            Self::Xp => "rxdata",
            Self::Vx => "rvdata",
            Self::VxAce => "rvdata2",
        }
    }

    pub fn encrypted_extension(&self) -> &'static str {
        match self {
            Self::Xp => ".rgssad",
            Self::Vx => ".rgss2a",
            Self::VxAce => ".rgss3a",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum MarshalValue {
    Nil,
    Bool(bool),
    Fixnum(i32),
    String(Vec<u8>),
    Symbol(String),
    Array(Vec<MarshalValue>),
    Hash(Vec<(MarshalValue, MarshalValue)>),
    Object {
        class_name: String,
        ivars: Vec<(String, MarshalValue)>,
    },
    UserDefined {
        class_name: String,
        data: Vec<u8>,
    },
    UserMarshal {
        class_name: String,
        value: Box<MarshalValue>,
    },
    InstanceVars {
        target: Box<MarshalValue>,
        ivars: Vec<(String, MarshalValue)>,
    },
}

impl MarshalValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(bytes) => std::str::from_utf8(bytes).ok(),
            Self::InstanceVars { target, .. } => target.as_str(),
            _ => None,
        }
    }

    pub fn as_i32(&self) -> Option<i32> {
        match self {
            Self::Fixnum(n) => Some(*n),
            Self::InstanceVars { target, .. } => target.as_i32(),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[MarshalValue]> {
        match self {
            Self::Array(arr) => Some(arr),
            Self::InstanceVars { target, .. } => target.as_array(),
            _ => None,
        }
    }

    pub fn get_ivar(&self, name: &str) -> Option<&MarshalValue> {
        let name_with_at = if name.starts_with('@') {
            name.to_string()
        } else {
            format!("@{name}")
        };
        let name_without_at = name.strip_prefix('@').unwrap_or(name);

        match self {
            Self::Object { ivars, .. } => {
                for (k, v) in ivars {
                    if k == &name_with_at || k == name_without_at {
                        return Some(v);
                    }
                }
                None
            }
            Self::InstanceVars { target, ivars } => {
                for (k, v) in ivars {
                    if k == &name_with_at || k == name_without_at {
                        return Some(v);
                    }
                }
                target.get_ivar(name)
            }
            _ => None,
        }
    }

    pub fn get_ivar_str(&self, name: &str) -> Option<&str> {
        self.get_ivar(name).and_then(|v| v.as_str())
    }

    pub fn get_ivar_i32(&self, name: &str) -> Option<i32> {
        self.get_ivar(name).and_then(|v| v.as_i32())
    }
}

#[derive(Debug)]
pub enum MarshalError {
    UnexpectedEof,
    BadMagic,
    Corrupt(String),
}

impl std::fmt::Display for MarshalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnexpectedEof => write!(f, "unexpected end of file"),
            Self::BadMagic => write!(f, "invalid Ruby Marshal header (expected 0x04 0x08)"),
            Self::Corrupt(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for MarshalError {}

struct MarshalReader<'a> {
    data: &'a [u8],
    pos: usize,
    symbols: Vec<String>,
    objects: Vec<MarshalValue>,
    depth: usize,
}

impl<'a> MarshalReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            pos: 0,
            symbols: Vec::new(),
            objects: Vec::new(),
            depth: 0,
        }
    }

    fn read_byte(&mut self) -> Result<u8, MarshalError> {
        if self.pos >= self.data.len() {
            return Err(MarshalError::UnexpectedEof);
        }
        let b = self.data[self.pos];
        self.pos += 1;
        Ok(b)
    }

    fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], MarshalError> {
        if self.pos + len > self.data.len() {
            return Err(MarshalError::UnexpectedEof);
        }
        let slice = &self.data[self.pos..self.pos + len];
        self.pos += len;
        Ok(slice)
    }

    fn read_long(&mut self) -> Result<i32, MarshalError> {
        let b = self.read_byte()? as i8;
        if b == 0 {
            return Ok(0);
        }
        if (4..=127).contains(&b) {
            return Ok((b - 5) as i32);
        }
        if (-128..=-4).contains(&b) {
            return Ok((b + 5) as i32);
        }
        if (1..=4).contains(&b) {
            let count = b as usize;
            let mut val = 0i32;
            for i in 0..count {
                let byte = self.read_byte()?;
                val |= (byte as i32) << (8 * i);
            }
            return Ok(val);
        }
        if (-4..=-1).contains(&b) {
            let count = (-b) as usize;
            let mut val = -1i32;
            for i in 0..count {
                let byte = self.read_byte()?;
                val = (val & !(0xff << (8 * i))) | ((byte as i32) << (8 * i));
            }
            return Ok(val);
        }
        Err(MarshalError::Corrupt(format!("invalid long prefix: {b}")))
    }

    fn read_symbol(&mut self) -> Result<String, MarshalError> {
        let token = self.read_byte()?;
        match token {
            b':' => {
                let len = self.read_long()?;
                if len < 0 {
                    return Err(MarshalError::Corrupt("negative symbol length".into()));
                }
                let bytes = self.read_bytes(len as usize)?;
                let sym = String::from_utf8_lossy(bytes).into_owned();
                self.symbols.push(sym.clone());
                Ok(sym)
            }
            b';' => {
                let idx = self.read_long()?;
                if idx < 0 || (idx as usize) >= self.symbols.len() {
                    return Err(MarshalError::Corrupt(format!(
                        "symbol link out of bounds: {idx}"
                    )));
                }
                Ok(self.symbols[idx as usize].clone())
            }
            other => Err(MarshalError::Corrupt(format!(
                "expected symbol, got byte {other:#x}"
            ))),
        }
    }

    fn parse_value(&mut self) -> Result<MarshalValue, MarshalError> {
        if self.depth > 256 {
            return Err(MarshalError::Corrupt("max recursion depth exceeded".into()));
        }
        self.depth += 1;
        let res = self.parse_value_inner();
        self.depth -= 1;
        res
    }

    fn parse_value_inner(&mut self) -> Result<MarshalValue, MarshalError> {
        let token = self.read_byte()?;
        match token {
            b'0' => Ok(MarshalValue::Nil),
            b'T' => Ok(MarshalValue::Bool(true)),
            b'F' => Ok(MarshalValue::Bool(false)),
            b'i' => {
                let n = self.read_long()?;
                Ok(MarshalValue::Fixnum(n))
            }
            b':' => {
                let len = self.read_long()?;
                if len < 0 {
                    return Err(MarshalError::Corrupt("negative symbol length".into()));
                }
                let bytes = self.read_bytes(len as usize)?;
                let sym = String::from_utf8_lossy(bytes).into_owned();
                self.symbols.push(sym.clone());
                Ok(MarshalValue::Symbol(sym))
            }
            b';' => {
                let idx = self.read_long()?;
                if idx < 0 || (idx as usize) >= self.symbols.len() {
                    return Err(MarshalError::Corrupt(format!(
                        "symbol link out of bounds: {idx}"
                    )));
                }
                Ok(MarshalValue::Symbol(self.symbols[idx as usize].clone()))
            }
            b'@' => {
                let idx = self.read_long()?;
                if idx < 0 || (idx as usize) >= self.objects.len() {
                    return Err(MarshalError::Corrupt(format!(
                        "object link out of bounds: {idx}"
                    )));
                }
                Ok(self.objects[idx as usize].clone())
            }
            b'"' => {
                let slot = self.objects.len();
                self.objects.push(MarshalValue::Nil);
                let len = self.read_long()?;
                if len < 0 {
                    return Err(MarshalError::Corrupt("negative string length".into()));
                }
                let bytes = self.read_bytes(len as usize)?.to_vec();
                let val = MarshalValue::String(bytes);
                self.objects[slot] = val.clone();
                Ok(val)
            }
            b'I' => {
                let target = self.parse_value()?;
                let ivar_count = self.read_long()?;
                if ivar_count < 0 {
                    return Err(MarshalError::Corrupt("negative ivar count".into()));
                }
                let mut ivars = Vec::with_capacity(ivar_count as usize);
                for _ in 0..ivar_count {
                    let sym = self.read_symbol()?;
                    let val = self.parse_value()?;
                    ivars.push((sym, val));
                }
                let res = MarshalValue::InstanceVars {
                    target: Box::new(target),
                    ivars,
                };
                Ok(res)
            }
            b'[' => {
                let slot = self.objects.len();
                self.objects.push(MarshalValue::Nil);
                let len = self.read_long()?;
                if len < 0 {
                    return Err(MarshalError::Corrupt("negative array length".into()));
                }
                let mut items = Vec::with_capacity(len as usize);
                for _ in 0..len {
                    items.push(self.parse_value()?);
                }
                let val = MarshalValue::Array(items);
                self.objects[slot] = val.clone();
                Ok(val)
            }
            b'{' => {
                let slot = self.objects.len();
                self.objects.push(MarshalValue::Nil);
                let len = self.read_long()?;
                if len < 0 {
                    return Err(MarshalError::Corrupt("negative hash length".into()));
                }
                let mut pairs = Vec::with_capacity(len as usize);
                for _ in 0..len {
                    let k = self.parse_value()?;
                    let v = self.parse_value()?;
                    pairs.push((k, v));
                }
                let val = MarshalValue::Hash(pairs);
                self.objects[slot] = val.clone();
                Ok(val)
            }
            b'o' => {
                let slot = self.objects.len();
                self.objects.push(MarshalValue::Nil);
                let class_name = self.read_symbol()?;
                let ivar_count = self.read_long()?;
                if ivar_count < 0 {
                    return Err(MarshalError::Corrupt("negative object ivar count".into()));
                }
                let mut ivars = Vec::with_capacity(ivar_count as usize);
                for _ in 0..ivar_count {
                    let k = self.read_symbol()?;
                    let v = self.parse_value()?;
                    ivars.push((k, v));
                }
                let val = MarshalValue::Object { class_name, ivars };
                self.objects[slot] = val.clone();
                Ok(val)
            }
            b'u' => {
                let slot = self.objects.len();
                self.objects.push(MarshalValue::Nil);
                let class_name = self.read_symbol()?;
                let len = self.read_long()?;
                if len < 0 {
                    return Err(MarshalError::Corrupt("negative user data length".into()));
                }
                let data = self.read_bytes(len as usize)?.to_vec();
                let val = MarshalValue::UserDefined { class_name, data };
                self.objects[slot] = val.clone();
                Ok(val)
            }
            b'U' => {
                let slot = self.objects.len();
                self.objects.push(MarshalValue::Nil);
                let class_name = self.read_symbol()?;
                let val = self.parse_value()?;
                let res = MarshalValue::UserMarshal {
                    class_name,
                    value: Box::new(val),
                };
                self.objects[slot] = res.clone();
                Ok(res)
            }
            b'f' => {
                let slot = self.objects.len();
                self.objects.push(MarshalValue::Nil);
                let len = self.read_long()?;
                if len < 0 {
                    return Err(MarshalError::Corrupt("negative float length".into()));
                }
                let bytes = self.read_bytes(len as usize)?;
                let s = String::from_utf8_lossy(bytes).into_owned();
                let val = MarshalValue::String(s.into_bytes());
                self.objects[slot] = val.clone();
                Ok(val)
            }
            b'l' => {
                let slot = self.objects.len();
                self.objects.push(MarshalValue::Nil);
                let _sign = self.read_byte()?;
                let len = self.read_long()?;
                if len < 0 {
                    return Err(MarshalError::Corrupt("negative bignum length".into()));
                }
                let bytes = self.read_bytes((len * 2) as usize)?.to_vec();
                let val = MarshalValue::String(bytes);
                self.objects[slot] = val.clone();
                Ok(val)
            }
            b'e' => {
                let _mod = self.read_symbol()?;
                self.parse_value()
            }
            b'C' => {
                let _cls = self.read_symbol()?;
                self.parse_value()
            }
            other => Err(MarshalError::Corrupt(format!(
                "unknown marshal token: {other:#x} ({})",
                other as char
            ))),
        }
    }
}

pub fn parse_marshal(data: &[u8]) -> Result<MarshalValue, MarshalError> {
    if data.len() < 2 {
        return Err(MarshalError::UnexpectedEof);
    }
    if data[0] != 0x04 || data[1] != 0x08 {
        return Err(MarshalError::BadMagic);
    }
    let mut reader = MarshalReader::new(&data[2..]);
    reader.parse_value()
}

pub struct RgssAssetAuditor;

impl Default for RgssAssetAuditor {
    fn default() -> Self {
        Self::new()
    }
}

impl RgssAssetAuditor {
    pub fn new() -> Self {
        Self
    }
}

const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "bmp"];
const AUDIO_EXTENSIONS: &[&str] = &["ogg", "mp3", "wav", "mid", "wma"];

impl AssetAuditor for RgssAssetAuditor {
    fn id(&self) -> &'static str {
        "rgss-static-auditor"
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn supports(&self, engine: EngineId) -> bool {
        matches!(
            engine,
            EngineId::RpgMakerXp | EngineId::RpgMakerVx | EngineId::RpgMakerVxAce
        )
    }

    fn audit(
        &self,
        source: &GameSourceIndex,
        engine: &EngineDescriptor,
        options: &AssetAuditOptions,
    ) -> AssetAuditReport {
        let gen = match RgssGeneration::for_engine(engine.id) {
            Some(g) => g,
            None => {
                let mut report = AssetAuditReport {
                    schema_version: 1,
                    engine: engine.id,
                    mode: options.mode,
                    outcome: AuditOutcome::Indeterminate,
                    completeness: AuditCoverage::new().to_completeness(),
                    statistics: AuditStatistics::default(),
                    graph: DependencyGraph::default(),
                    findings: vec![make_finding(
                        DiagnosticCode::CMP_AUDITOR_UNAVAILABLE,
                        ReferenceRequiredness::Required,
                        format!(
                            "RGSS static auditor does not support engine {:?}",
                            engine.id
                        ),
                        Vec::new(),
                    )],
                    overrides: Vec::new(),
                    runtime_validation: RuntimeValidationState::NotRun,
                };
                report.canonicalize();
                report.recompute_outcome();
                return report;
            }
        };

        let mut findings = Vec::new();
        let mut graph = DependencyGraph::default();
        let mut coverage = AuditCoverage::new();
        let resolver = AssetResolver::new(source);

        // Check limits
        if source.files.len() as u64 > options.limits.max_files
            || source.total_size_bytes > options.limits.max_total_bytes
        {
            findings.push(make_finding(
                DiagnosticCode::CMP_AUD_LIMIT_REACHED,
                ReferenceRequiredness::Required,
                format!(
                    "Audit limits exceeded: total files {}, total bytes {}",
                    source.files.len(),
                    source.total_size_bytes
                ),
                Vec::new(),
            ));
        }

        // Declare covered reference classes in standard mode
        coverage.add_covered("system");
        coverage.add_covered("maps");
        coverage.add_covered("events");
        coverage.add_covered("database");

        let ext = gen.data_extension();

        // 1. Audit System data
        let system_path = format!("Data/System.{ext}");
        audit_system_file(
            &system_path,
            gen,
            source,
            &resolver,
            &mut graph,
            &mut findings,
        );

        // 2. Audit Database files
        audit_database_files(gen, source, &resolver, &mut graph, &mut findings);

        // 3. Audit Map files
        audit_map_files(gen, source, &resolver, &mut graph, &mut findings);

        // 4. Standard vs Deep mode checks
        let has_encrypted_archive = source.files.values().any(|f| {
            f.logical_path
                .to_ascii_lowercase()
                .ends_with(gen.encrypted_extension())
        });

        if has_encrypted_archive {
            let archive_path = source
                .files
                .values()
                .find(|f| {
                    f.logical_path
                        .to_ascii_lowercase()
                        .ends_with(gen.encrypted_extension())
                })
                .map(|f| f.logical_path.clone())
                .unwrap_or_default();

            if options.mode == AuditMode::Deep {
                coverage.add_unsupported("encrypted_archive", ReferenceRequiredness::Required);
                findings.push(make_finding(
                    DiagnosticCode::CMP_AUD_DYNAMIC_REFERENCE_UNRESOLVED,
                    ReferenceRequiredness::Required,
                    format!("Encrypted RGSS archive '{archive_path}' requires decryption key and runtime extraction for static inventory"),
                    vec![archive_path],
                ));
            } else if !source.has_file(&system_path) {
                // In standard mode, if Data/System does not exist unpacked because it is in an encrypted archive
                coverage.add_unsupported("encrypted_archive", ReferenceRequiredness::Required);
                findings.push(make_finding(
                    DiagnosticCode::CMP_AUD_DYNAMIC_REFERENCE_UNRESOLVED,
                    ReferenceRequiredness::Required,
                    format!("Game content is stored in encrypted archive '{archive_path}' without unpacked data files"),
                    vec![archive_path],
                ));
            }
        }

        if options.mode == AuditMode::Deep {
            let scripts_path = format!("Data/Scripts.{ext}");
            if let Some(indexed_file) = source.get_file(&scripts_path) {
                let full_path = source.root_path.join(&indexed_file.relative_path);
                match std::fs::read(&full_path) {
                    Ok(bytes) => match parse_marshal(&bytes) {
                        Ok(_) => {
                            coverage.add_unsupported("scripts", ReferenceRequiredness::Required);
                            findings.push(make_finding(
                                DiagnosticCode::CMP_AUD_DYNAMIC_REFERENCE_UNRESOLVED,
                                ReferenceRequiredness::Required,
                                format!("RGSS script archive '{scripts_path}' contains dynamic Ruby code that cannot be fully verified statically"),
                                vec![scripts_path],
                            ));
                        }
                        Err(e) => {
                            findings.push(make_finding(
                                DiagnosticCode::CMP_AUD_MALFORMED_RESOURCE,
                                ReferenceRequiredness::Required,
                                format!(
                                    "Malformed or corrupt script archive '{scripts_path}': {e}"
                                ),
                                vec![scripts_path],
                            ));
                        }
                    },
                    Err(e) => {
                        findings.push(make_finding(
                            DiagnosticCode::CMP_AUD_MALFORMED_RESOURCE,
                            ReferenceRequiredness::Required,
                            format!("Failed to read script archive '{scripts_path}': {e}"),
                            vec![scripts_path],
                        ));
                    }
                }
            } else {
                coverage.add_unsupported("scripts", ReferenceRequiredness::Required);
            }
        }

        let mut report = AssetAuditReport {
            schema_version: 1,
            engine: engine.id,
            mode: options.mode,
            outcome: AuditOutcome::Pass,
            completeness: coverage.to_completeness(),
            statistics: AuditStatistics::default(),
            graph,
            findings,
            overrides: Vec::new(),
            runtime_validation: RuntimeValidationState::NotRun,
        };
        report.canonicalize();
        report.recompute_outcome();
        report
    }
}

fn read_and_parse_data_file(
    logical_path: &str,
    source: &GameSourceIndex,
    findings: &mut Vec<AuditFinding>,
) -> Option<MarshalValue> {
    let indexed_file = source.get_file(logical_path)?;
    let full_path = source.root_path.join(&indexed_file.relative_path);
    let bytes = match std::fs::read(&full_path) {
        Ok(b) => b,
        Err(e) => {
            findings.push(make_finding(
                DiagnosticCode::CMP_AUD_MALFORMED_RESOURCE,
                ReferenceRequiredness::Required,
                format!("Failed to read data file '{logical_path}': {e}"),
                vec![logical_path.to_string()],
            ));
            return None;
        }
    };
    match parse_marshal(&bytes) {
        Ok(val) => Some(val),
        Err(e) => {
            findings.push(make_finding(
                DiagnosticCode::CMP_AUD_MALFORMED_RESOURCE,
                ReferenceRequiredness::Required,
                format!("Malformed or corrupt data file '{logical_path}': {e}"),
                vec![logical_path.to_string()],
            ));
            None
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_and_record(
    resolver: &AssetResolver,
    graph: &mut DependencyGraph,
    findings: &mut Vec<AuditFinding>,
    source_node_id: &str,
    target_base_path: &str,
    extensions: &[&str],
    kind: AssetKind,
    requiredness: ReferenceRequiredness,
) {
    let rule = CandidateRule::with_extensions(target_base_path, extensions.iter().copied());
    let res = resolver.resolve(&rule, kind);

    let (target_id, resolution, target_logical_path) = match &res {
        ResolutionResult::Exact(node) => (
            node.id.clone(),
            ReferenceResolution::Resolved,
            node.logical_path.clone(),
        ),
        ResolutionResult::CaseMismatch {
            node,
            expected,
            actual,
        } => {
            findings.push(make_finding(
                DiagnosticCode::CMP_AUD_CASE_MISMATCH,
                ReferenceRequiredness::Required,
                format!("Asset reference '{expected}' has case mismatch with '{actual}' on disk"),
                vec![source_node_id.to_string()],
            ));
            (
                node.id.clone(),
                ReferenceResolution::Resolved,
                node.logical_path.clone(),
            )
        }
        ResolutionResult::CaseCollision(colliding) => {
            findings.push(make_finding(
                DiagnosticCode::CMP_AUD_CASE_COLLISION,
                ReferenceRequiredness::Required,
                format!(
                    "Asset reference '{target_base_path}' collides case-insensitively with multiple files: {:?}",
                    colliding
                ),
                vec![source_node_id.to_string()],
            ));
            (
                format!("{}:{target_base_path}", kind.as_str()),
                ReferenceResolution::Unresolved,
                None,
            )
        }
        ResolutionResult::Missing(attempted) => {
            let code = if requiredness == ReferenceRequiredness::Required {
                DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING
            } else {
                DiagnosticCode::CMP_AUD_OPTIONAL_REFERENCE_MISSING
            };
            findings.push(make_finding(
                code,
                requiredness,
                format!(
                    "Referenced asset '{target_base_path}' is missing on disk (attempted: {:?})",
                    attempted
                ),
                vec![source_node_id.to_string()],
            ));
            (
                format!("{}:{target_base_path}", kind.as_str()),
                ReferenceResolution::Unresolved,
                None,
            )
        }
        ResolutionResult::Unsafe(path) => {
            findings.push(make_finding(
                DiagnosticCode::CMP_AUD_UNSAFE_PATH,
                ReferenceRequiredness::Required,
                format!("Unsafe path traversal in asset reference: '{path}'"),
                vec![source_node_id.to_string()],
            ));
            (
                format!("{}:{target_base_path}", kind.as_str()),
                ReferenceResolution::Unresolved,
                None,
            )
        }
    };

    if !graph.nodes.iter().any(|n| n.id == target_id) {
        graph.nodes.push(AssetNode {
            id: target_id.clone(),
            kind,
            logical_path: target_logical_path,
        });
    }

    graph.edges.push(ReferenceEdge {
        source: source_node_id.to_string(),
        target: target_id,
        relation: "references".to_string(),
        requiredness,
        evidence: match resolution {
            ReferenceResolution::Resolved => EvidenceTrail::through(EvidenceStage::FileExists),
            _ => EvidenceTrail::through(EvidenceStage::ReferenceFound),
        },
        resolution,
    });
}

fn audit_audio_val(
    audio: &MarshalValue,
    folder: &str,
    source_node: &str,
    resolver: &AssetResolver,
    graph: &mut DependencyGraph,
    findings: &mut Vec<AuditFinding>,
) {
    if let Some(name) = audio.get_ivar_str("@name").or_else(|| audio.as_str()) {
        if !name.is_empty() {
            let base = format!("{folder}/{name}");
            resolve_and_record(
                resolver,
                graph,
                findings,
                source_node,
                &base,
                AUDIO_EXTENSIONS,
                AssetKind::Audio,
                ReferenceRequiredness::Required,
            );
        }
    }
}

fn audit_system_file(
    logical_path: &str,
    gen: RgssGeneration,
    source: &GameSourceIndex,
    resolver: &AssetResolver,
    graph: &mut DependencyGraph,
    findings: &mut Vec<AuditFinding>,
) {
    let sys = match read_and_parse_data_file(logical_path, source, findings) {
        Some(s) => s,
        None => return,
    };
    let sys_node_id = "Database:System".to_string();
    if !graph.nodes.iter().any(|n| n.id == sys_node_id) {
        graph.nodes.push(AssetNode {
            id: sys_node_id.clone(),
            kind: AssetKind::DatabaseEntity,
            logical_path: Some(logical_path.to_string()),
        });
    }

    match gen {
        RgssGeneration::Xp => {
            for (field, folder) in [
                ("@title_name", "Graphics/Titles"),
                ("@windowskin_name", "Graphics/Windowskins"),
                ("@gameover_name", "Graphics/Gameovers"),
                ("@transition_name", "Graphics/Transitions"),
                ("@battleback_name", "Graphics/Battlebacks"),
                ("@battler_name", "Graphics/Battlers"),
            ] {
                if let Some(name) = sys.get_ivar_str(field) {
                    if !name.is_empty() {
                        resolve_and_record(
                            resolver,
                            graph,
                            findings,
                            &sys_node_id,
                            &format!("{folder}/{name}"),
                            IMAGE_EXTENSIONS,
                            AssetKind::Image,
                            ReferenceRequiredness::Required,
                        );
                    }
                }
            }

            for field in ["@title_bgm", "@battle_bgm"] {
                if let Some(audio) = sys.get_ivar(field) {
                    audit_audio_val(audio, "Audio/BGM", &sys_node_id, resolver, graph, findings);
                }
            }
            for field in ["@battle_end_me", "@gameover_me"] {
                if let Some(audio) = sys.get_ivar(field) {
                    audit_audio_val(audio, "Audio/ME", &sys_node_id, resolver, graph, findings);
                }
            }
            for field in [
                "@cursor_se",
                "@decision_se",
                "@cancel_se",
                "@buzzer_se",
                "@equip_se",
                "@shop_se",
                "@save_se",
                "@load_se",
                "@battle_start_se",
                "@escape_se",
                "@actor_collapse_se",
                "@enemy_collapse_se",
            ] {
                if let Some(audio) = sys.get_ivar(field) {
                    audit_audio_val(audio, "Audio/SE", &sys_node_id, resolver, graph, findings);
                }
            }
        }
        RgssGeneration::Vx => {
            if let Some(name) = sys.get_ivar_str("@gameover_name") {
                if !name.is_empty() {
                    resolve_and_record(
                        resolver,
                        graph,
                        findings,
                        &sys_node_id,
                        &format!("Graphics/System/{name}"),
                        IMAGE_EXTENSIONS,
                        AssetKind::Image,
                        ReferenceRequiredness::Required,
                    );
                }
            }
            for field in ["@title_bgm", "@battle_bgm"] {
                if let Some(audio) = sys.get_ivar(field) {
                    audit_audio_val(audio, "Audio/BGM", &sys_node_id, resolver, graph, findings);
                }
            }
            for field in ["@battle_end_me", "@gameover_me"] {
                if let Some(audio) = sys.get_ivar(field) {
                    audit_audio_val(audio, "Audio/ME", &sys_node_id, resolver, graph, findings);
                }
            }
            if let Some(sounds) = sys.get_ivar("@sounds").and_then(|v| v.as_array()) {
                for sound in sounds {
                    audit_audio_val(sound, "Audio/SE", &sys_node_id, resolver, graph, findings);
                }
            }
            for v_name in ["@boat", "@ship", "@airship"] {
                if let Some(vehicle) = sys.get_ivar(v_name) {
                    if let Some(c_name) = vehicle.get_ivar_str("@character_name") {
                        if !c_name.is_empty() {
                            resolve_and_record(
                                resolver,
                                graph,
                                findings,
                                &sys_node_id,
                                &format!("Graphics/Characters/{c_name}"),
                                IMAGE_EXTENSIONS,
                                AssetKind::Image,
                                ReferenceRequiredness::Required,
                            );
                        }
                    }
                    if let Some(bgm) = vehicle.get_ivar("@bgm") {
                        audit_audio_val(bgm, "Audio/BGM", &sys_node_id, resolver, graph, findings);
                    }
                }
            }
        }
        RgssGeneration::VxAce => {
            for (field, folder) in [
                ("@title1_name", "Graphics/Titles1"),
                ("@title2_name", "Graphics/Titles2"),
                ("@gameover_name", "Graphics/Gameovers"),
                ("@windowskin_name", "Graphics/System"),
            ] {
                if let Some(name) = sys.get_ivar_str(field) {
                    if !name.is_empty() {
                        resolve_and_record(
                            resolver,
                            graph,
                            findings,
                            &sys_node_id,
                            &format!("{folder}/{name}"),
                            IMAGE_EXTENSIONS,
                            AssetKind::Image,
                            ReferenceRequiredness::Required,
                        );
                    }
                }
            }
            for field in ["@title_bgm", "@battle_bgm"] {
                if let Some(audio) = sys.get_ivar(field) {
                    audit_audio_val(audio, "Audio/BGM", &sys_node_id, resolver, graph, findings);
                }
            }
            for field in ["@battle_end_me", "@gameover_me"] {
                if let Some(audio) = sys.get_ivar(field) {
                    audit_audio_val(audio, "Audio/ME", &sys_node_id, resolver, graph, findings);
                }
            }
            if let Some(sounds) = sys.get_ivar("@sounds").and_then(|v| v.as_array()) {
                for sound in sounds {
                    audit_audio_val(sound, "Audio/SE", &sys_node_id, resolver, graph, findings);
                }
            }
            for v_name in ["@boat", "@ship", "@airship"] {
                if let Some(vehicle) = sys.get_ivar(v_name) {
                    if let Some(c_name) = vehicle.get_ivar_str("@character_name") {
                        if !c_name.is_empty() {
                            resolve_and_record(
                                resolver,
                                graph,
                                findings,
                                &sys_node_id,
                                &format!("Graphics/Characters/{c_name}"),
                                IMAGE_EXTENSIONS,
                                AssetKind::Image,
                                ReferenceRequiredness::Required,
                            );
                        }
                    }
                    if let Some(bgm) = vehicle.get_ivar("@bgm") {
                        audit_audio_val(bgm, "Audio/BGM", &sys_node_id, resolver, graph, findings);
                    }
                }
            }
        }
    }
}

fn audit_database_files(
    gen: RgssGeneration,
    source: &GameSourceIndex,
    resolver: &AssetResolver,
    graph: &mut DependencyGraph,
    findings: &mut Vec<AuditFinding>,
) {
    let ext = gen.data_extension();

    // Actors
    let actors_path = format!("Data/Actors.{ext}");
    if let Some(val) = read_and_parse_data_file(&actors_path, source, findings) {
        if let Some(actors) = val.as_array() {
            for actor in actors {
                let id = actor.get_ivar_i32("@id").unwrap_or(0);
                if id == 0 {
                    continue;
                }
                let node_id = format!("Database:Actors:{id}");
                if !graph.nodes.iter().any(|n| n.id == node_id) {
                    graph.nodes.push(AssetNode {
                        id: node_id.clone(),
                        kind: AssetKind::DatabaseEntity,
                        logical_path: None,
                    });
                }
                if let Some(c_name) = actor.get_ivar_str("@character_name") {
                    if !c_name.is_empty() {
                        resolve_and_record(
                            resolver,
                            graph,
                            findings,
                            &node_id,
                            &format!("Graphics/Characters/{c_name}"),
                            IMAGE_EXTENSIONS,
                            AssetKind::Image,
                            ReferenceRequiredness::Required,
                        );
                    }
                }
                match gen {
                    RgssGeneration::Xp => {
                        if let Some(b_name) = actor.get_ivar_str("@battler_name") {
                            if !b_name.is_empty() {
                                resolve_and_record(
                                    resolver,
                                    graph,
                                    findings,
                                    &node_id,
                                    &format!("Graphics/Battlers/{b_name}"),
                                    IMAGE_EXTENSIONS,
                                    AssetKind::Image,
                                    ReferenceRequiredness::Required,
                                );
                            }
                        }
                    }
                    RgssGeneration::Vx | RgssGeneration::VxAce => {
                        if let Some(f_name) = actor.get_ivar_str("@face_name") {
                            if !f_name.is_empty() {
                                resolve_and_record(
                                    resolver,
                                    graph,
                                    findings,
                                    &node_id,
                                    &format!("Graphics/Faces/{f_name}"),
                                    IMAGE_EXTENSIONS,
                                    AssetKind::Image,
                                    ReferenceRequiredness::Required,
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    // Enemies
    let enemies_path = format!("Data/Enemies.{ext}");
    if let Some(val) = read_and_parse_data_file(&enemies_path, source, findings) {
        if let Some(enemies) = val.as_array() {
            for enemy in enemies {
                let id = enemy.get_ivar_i32("@id").unwrap_or(0);
                if id == 0 {
                    continue;
                }
                let node_id = format!("Database:Enemies:{id}");
                if !graph.nodes.iter().any(|n| n.id == node_id) {
                    graph.nodes.push(AssetNode {
                        id: node_id.clone(),
                        kind: AssetKind::DatabaseEntity,
                        logical_path: None,
                    });
                }
                if let Some(b_name) = enemy.get_ivar_str("@battler_name") {
                    if !b_name.is_empty() {
                        resolve_and_record(
                            resolver,
                            graph,
                            findings,
                            &node_id,
                            &format!("Graphics/Battlers/{b_name}"),
                            IMAGE_EXTENSIONS,
                            AssetKind::Image,
                            ReferenceRequiredness::Required,
                        );
                    }
                }
            }
        }
    }

    // Tilesets
    let tilesets_path = format!("Data/Tilesets.{ext}");
    if let Some(val) = read_and_parse_data_file(&tilesets_path, source, findings) {
        if let Some(tilesets) = val.as_array() {
            for ts in tilesets {
                let id = ts.get_ivar_i32("@id").unwrap_or(0);
                if id == 0 {
                    continue;
                }
                let node_id = format!("Database:Tilesets:{id}");
                if !graph.nodes.iter().any(|n| n.id == node_id) {
                    graph.nodes.push(AssetNode {
                        id: node_id.clone(),
                        kind: AssetKind::DatabaseEntity,
                        logical_path: None,
                    });
                }
                match gen {
                    RgssGeneration::Xp => {
                        if let Some(ts_name) = ts.get_ivar_str("@tileset_name") {
                            if !ts_name.is_empty() {
                                resolve_and_record(
                                    resolver,
                                    graph,
                                    findings,
                                    &node_id,
                                    &format!("Graphics/Tilesets/{ts_name}"),
                                    IMAGE_EXTENSIONS,
                                    AssetKind::Image,
                                    ReferenceRequiredness::Required,
                                );
                            }
                        }
                        if let Some(autotiles) =
                            ts.get_ivar("@autotile_names").and_then(|v| v.as_array())
                        {
                            for at in autotiles {
                                if let Some(at_name) = at.as_str() {
                                    if !at_name.is_empty() {
                                        resolve_and_record(
                                            resolver,
                                            graph,
                                            findings,
                                            &node_id,
                                            &format!("Graphics/Autotiles/{at_name}"),
                                            IMAGE_EXTENSIONS,
                                            AssetKind::Image,
                                            ReferenceRequiredness::Required,
                                        );
                                    }
                                }
                            }
                        }
                        for (field, folder) in [
                            ("@panorama_name", "Graphics/Panoramas"),
                            ("@fog_name", "Graphics/Fogs"),
                            ("@battleback_name", "Graphics/Battlebacks"),
                        ] {
                            if let Some(name) = ts.get_ivar_str(field) {
                                if !name.is_empty() {
                                    resolve_and_record(
                                        resolver,
                                        graph,
                                        findings,
                                        &node_id,
                                        &format!("{folder}/{name}"),
                                        IMAGE_EXTENSIONS,
                                        AssetKind::Image,
                                        ReferenceRequiredness::Required,
                                    );
                                }
                            }
                        }
                    }
                    RgssGeneration::Vx => {}
                    RgssGeneration::VxAce => {
                        if let Some(tileset_names) =
                            ts.get_ivar("@tileset_names").and_then(|v| v.as_array())
                        {
                            for name_val in tileset_names {
                                if let Some(name) = name_val.as_str() {
                                    if !name.is_empty() {
                                        resolve_and_record(
                                            resolver,
                                            graph,
                                            findings,
                                            &node_id,
                                            &format!("Graphics/Tilesets/{name}"),
                                            IMAGE_EXTENSIONS,
                                            AssetKind::Image,
                                            ReferenceRequiredness::Required,
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Animations
    let anims_path = format!("Data/Animations.{ext}");
    if let Some(val) = read_and_parse_data_file(&anims_path, source, findings) {
        if let Some(anims) = val.as_array() {
            for anim in anims {
                let id = anim.get_ivar_i32("@id").unwrap_or(0);
                if id == 0 {
                    continue;
                }
                let node_id = format!("Database:Animations:{id}");
                if !graph.nodes.iter().any(|n| n.id == node_id) {
                    graph.nodes.push(AssetNode {
                        id: node_id.clone(),
                        kind: AssetKind::DatabaseEntity,
                        logical_path: None,
                    });
                }
                match gen {
                    RgssGeneration::Xp => {
                        if let Some(name) = anim.get_ivar_str("@animation_name") {
                            if !name.is_empty() {
                                resolve_and_record(
                                    resolver,
                                    graph,
                                    findings,
                                    &node_id,
                                    &format!("Graphics/Animations/{name}"),
                                    IMAGE_EXTENSIONS,
                                    AssetKind::Image,
                                    ReferenceRequiredness::Required,
                                );
                            }
                        }
                    }
                    RgssGeneration::Vx | RgssGeneration::VxAce => {
                        for field in ["@animation1_name", "@animation2_name"] {
                            if let Some(name) = anim.get_ivar_str(field) {
                                if !name.is_empty() {
                                    resolve_and_record(
                                        resolver,
                                        graph,
                                        findings,
                                        &node_id,
                                        &format!("Graphics/Animations/{name}"),
                                        IMAGE_EXTENSIONS,
                                        AssetKind::Image,
                                        ReferenceRequiredness::Required,
                                    );
                                }
                            }
                        }
                    }
                }
                if let Some(timings) = anim.get_ivar("@timings").and_then(|v| v.as_array()) {
                    for timing in timings {
                        if let Some(se) = timing.get_ivar("@se") {
                            audit_audio_val(se, "Audio/SE", &node_id, resolver, graph, findings);
                        }
                    }
                }
            }
        }
    }

    // CommonEvents
    let ce_path = format!("Data/CommonEvents.{ext}");
    if let Some(val) = read_and_parse_data_file(&ce_path, source, findings) {
        if let Some(ces) = val.as_array() {
            for ce in ces {
                let id = ce.get_ivar_i32("@id").unwrap_or(0);
                if id == 0 {
                    continue;
                }
                let node_id = format!("Database:CommonEvents:{id}");
                if !graph.nodes.iter().any(|n| n.id == node_id) {
                    graph.nodes.push(AssetNode {
                        id: node_id.clone(),
                        kind: AssetKind::DatabaseEntity,
                        logical_path: None,
                    });
                }
                if let Some(list) = ce.get_ivar("@list").and_then(|v| v.as_array()) {
                    audit_event_commands(list, &node_id, gen, resolver, graph, findings);
                }
            }
        }
    }

    // Troops
    let troops_path = format!("Data/Troops.{ext}");
    if let Some(val) = read_and_parse_data_file(&troops_path, source, findings) {
        if let Some(troops) = val.as_array() {
            for troop in troops {
                let id = troop.get_ivar_i32("@id").unwrap_or(0);
                if id == 0 {
                    continue;
                }
                let node_id = format!("Database:Troops:{id}");
                if !graph.nodes.iter().any(|n| n.id == node_id) {
                    graph.nodes.push(AssetNode {
                        id: node_id.clone(),
                        kind: AssetKind::DatabaseEntity,
                        logical_path: None,
                    });
                }
                if let Some(pages) = troop.get_ivar("@pages").and_then(|v| v.as_array()) {
                    for (page_idx, page) in pages.iter().enumerate() {
                        let page_node_id = format!("{node_id}:Page{page_idx}");
                        if !graph.nodes.iter().any(|n| n.id == page_node_id) {
                            graph.nodes.push(AssetNode {
                                id: page_node_id.clone(),
                                kind: AssetKind::EventPage,
                                logical_path: None,
                            });
                        }
                        if let Some(list) = page.get_ivar("@list").and_then(|v| v.as_array()) {
                            audit_event_commands(
                                list,
                                &page_node_id,
                                gen,
                                resolver,
                                graph,
                                findings,
                            );
                        }
                    }
                }
            }
        }
    }
}

fn audit_map_files(
    gen: RgssGeneration,
    source: &GameSourceIndex,
    resolver: &AssetResolver,
    graph: &mut DependencyGraph,
    findings: &mut Vec<AuditFinding>,
) {
    let ext = gen.data_extension();
    let mut map_paths: Vec<String> = source
        .files
        .values()
        .filter_map(|f| {
            let lower = f.logical_path.to_ascii_lowercase();
            if lower.starts_with("data/map") && lower.ends_with(&format!(".{ext}")) {
                Some(f.logical_path.clone())
            } else {
                None
            }
        })
        .collect();
    map_paths.sort();

    for map_path in map_paths {
        let map_obj = match read_and_parse_data_file(&map_path, source, findings) {
            Some(m) => m,
            None => continue,
        };

        // Extract map identifier e.g. "Map001"
        let filename = map_path.rsplit('/').next().unwrap_or(&map_path);
        let map_name = filename
            .strip_suffix(&format!(".{ext}"))
            .unwrap_or(filename);
        let map_node_id = map_name.to_string();

        if !graph.nodes.iter().any(|n| n.id == map_node_id) {
            graph.nodes.push(AssetNode {
                id: map_node_id.clone(),
                kind: AssetKind::Map,
                logical_path: Some(map_path.clone()),
            });
        }

        // BGM / BGS
        if let Some(bgm) = map_obj.get_ivar("@bgm") {
            audit_audio_val(bgm, "Audio/BGM", &map_node_id, resolver, graph, findings);
        }
        if let Some(bgs) = map_obj.get_ivar("@bgs") {
            audit_audio_val(bgs, "Audio/BGS", &map_node_id, resolver, graph, findings);
        }

        // Parallax / Panorama / Fog / Battleback
        match gen {
            RgssGeneration::Xp => {
                for (field, folder) in [
                    ("@panorama_name", "Graphics/Panoramas"),
                    ("@fog_name", "Graphics/Fogs"),
                    ("@battleback_name", "Graphics/Battlebacks"),
                ] {
                    if let Some(name) = map_obj.get_ivar_str(field) {
                        if !name.is_empty() {
                            resolve_and_record(
                                resolver,
                                graph,
                                findings,
                                &map_node_id,
                                &format!("{folder}/{name}"),
                                IMAGE_EXTENSIONS,
                                AssetKind::Image,
                                ReferenceRequiredness::Required,
                            );
                        }
                    }
                }
            }
            RgssGeneration::Vx => {
                if let Some(name) = map_obj.get_ivar_str("@parallax_name") {
                    if !name.is_empty() {
                        resolve_and_record(
                            resolver,
                            graph,
                            findings,
                            &map_node_id,
                            &format!("Graphics/Parallaxes/{name}"),
                            IMAGE_EXTENSIONS,
                            AssetKind::Image,
                            ReferenceRequiredness::Required,
                        );
                    }
                }
            }
            RgssGeneration::VxAce => {
                for (field, folder) in [
                    ("@parallax_name", "Graphics/Parallaxes"),
                    ("@battleback1_name", "Graphics/Battlebacks1"),
                    ("@battleback2_name", "Graphics/Battlebacks2"),
                ] {
                    if let Some(name) = map_obj.get_ivar_str(field) {
                        if !name.is_empty() {
                            resolve_and_record(
                                resolver,
                                graph,
                                findings,
                                &map_node_id,
                                &format!("{folder}/{name}"),
                                IMAGE_EXTENSIONS,
                                AssetKind::Image,
                                ReferenceRequiredness::Required,
                            );
                        }
                    }
                }
            }
        }

        // Events
        if let Some(events_val) = map_obj.get_ivar("@events") {
            let events_list: Vec<(i32, &MarshalValue)> = match events_val {
                MarshalValue::Hash(pairs) => pairs
                    .iter()
                    .filter_map(|(k, v)| k.as_i32().map(|id| (id, v)))
                    .collect(),
                MarshalValue::Array(arr) => arr
                    .iter()
                    .enumerate()
                    .map(|(idx, v)| (idx as i32, v))
                    .collect(),
                _ => Vec::new(),
            };

            for (event_id, event) in events_list {
                if event_id == 0 {
                    continue;
                }
                let event_node_id = format!("{map_node_id}:Event{event_id}");
                if !graph.nodes.iter().any(|n| n.id == event_node_id) {
                    graph.nodes.push(AssetNode {
                        id: event_node_id.clone(),
                        kind: AssetKind::Event,
                        logical_path: None,
                    });
                }

                if let Some(pages) = event.get_ivar("@pages").and_then(|v| v.as_array()) {
                    for (page_idx, page) in pages.iter().enumerate() {
                        let page_node_id = format!("{event_node_id}:Page{}", page_idx + 1);
                        if !graph.nodes.iter().any(|n| n.id == page_node_id) {
                            graph.nodes.push(AssetNode {
                                id: page_node_id.clone(),
                                kind: AssetKind::EventPage,
                                logical_path: None,
                            });
                        }

                        // Event graphic
                        if let Some(graphic) = page.get_ivar("@graphic") {
                            if let Some(char_name) = graphic.get_ivar_str("@character_name") {
                                if !char_name.is_empty() {
                                    resolve_and_record(
                                        resolver,
                                        graph,
                                        findings,
                                        &page_node_id,
                                        &format!("Graphics/Characters/{char_name}"),
                                        IMAGE_EXTENSIONS,
                                        AssetKind::Image,
                                        ReferenceRequiredness::Required,
                                    );
                                }
                            }
                        }

                        // Move route
                        if let Some(move_route) = page.get_ivar("@move_route") {
                            if let Some(move_cmds) =
                                move_route.get_ivar("@list").and_then(|v| v.as_array())
                            {
                                for mcmd in move_cmds {
                                    let mcode = mcmd.get_ivar_i32("@code").unwrap_or(0);
                                    let mparams = mcmd
                                        .get_ivar("@parameters")
                                        .and_then(|v| v.as_array())
                                        .unwrap_or(&[]);
                                    // 41: Change Graphic
                                    if mcode == 41 {
                                        if let Some(g_name) =
                                            mparams.first().and_then(|v| v.as_str())
                                        {
                                            if !g_name.is_empty() {
                                                resolve_and_record(
                                                    resolver,
                                                    graph,
                                                    findings,
                                                    &page_node_id,
                                                    &format!("Graphics/Characters/{g_name}"),
                                                    IMAGE_EXTENSIONS,
                                                    AssetKind::Image,
                                                    ReferenceRequiredness::Required,
                                                );
                                            }
                                        }
                                    }
                                    // 44: Play SE
                                    if mcode == 44 {
                                        if let Some(audio) = mparams.first() {
                                            audit_audio_val(
                                                audio,
                                                "Audio/SE",
                                                &page_node_id,
                                                resolver,
                                                graph,
                                                findings,
                                            );
                                        }
                                    }
                                }
                            }
                        }

                        // Event commands
                        if let Some(list) = page.get_ivar("@list").and_then(|v| v.as_array()) {
                            audit_event_commands(
                                list,
                                &page_node_id,
                                gen,
                                resolver,
                                graph,
                                findings,
                            );
                        }
                    }
                }
            }
        }
    }
}

fn audit_event_commands(
    commands: &[MarshalValue],
    provenance_prefix: &str,
    gen: RgssGeneration,
    resolver: &AssetResolver,
    graph: &mut DependencyGraph,
    findings: &mut Vec<AuditFinding>,
) {
    for (cmd_idx, cmd) in commands.iter().enumerate() {
        let code = cmd.get_ivar_i32("@code").unwrap_or(0);
        if code == 0 {
            continue;
        }
        let params = cmd
            .get_ivar("@parameters")
            .and_then(|v| v.as_array())
            .unwrap_or(&[]);
        let cmd_node_id = format!("{provenance_prefix}:Command{cmd_idx}");

        if !graph.nodes.iter().any(|n| n.id == cmd_node_id) {
            graph.nodes.push(AssetNode {
                id: cmd_node_id.clone(),
                kind: AssetKind::Command,
                logical_path: None,
            });
        }

        match code {
            // 201: Player Transfer
            201 => {
                let appointment_method = params.first().and_then(|v| v.as_i32()).unwrap_or(0);
                let map_id = params.get(1).and_then(|v| v.as_i32()).unwrap_or(0);
                if appointment_method == 0 {
                    // Direct transfer: map_id is static map reference
                    let ext = gen.data_extension();
                    let target_map_path = format!("Data/Map{:03}.{ext}", map_id);
                    let res = resolver.resolve_path(&target_map_path, AssetKind::Map);
                    let target_id = format!("map:{target_map_path}");
                    match res {
                        ResolutionResult::Exact(node) => {
                            if !graph.nodes.iter().any(|n| n.id == node.id) {
                                graph.nodes.push(node.clone());
                            }
                            graph.edges.push(ReferenceEdge {
                                source: cmd_node_id.clone(),
                                target: node.id,
                                relation: "transfer_target".into(),
                                requiredness: ReferenceRequiredness::Required,
                                evidence: EvidenceTrail::through(EvidenceStage::FileExists),
                                resolution: ReferenceResolution::Resolved,
                            });
                        }
                        _ => {
                            if !graph.nodes.iter().any(|n| n.id == target_id) {
                                graph.nodes.push(AssetNode {
                                    id: target_id.clone(),
                                    kind: AssetKind::Map,
                                    logical_path: Some(target_map_path.clone()),
                                });
                            }
                            graph.edges.push(ReferenceEdge {
                                source: cmd_node_id.clone(),
                                target: target_id,
                                relation: "transfer_target".into(),
                                requiredness: ReferenceRequiredness::Required,
                                evidence: EvidenceTrail::through(EvidenceStage::ReferenceFound),
                                resolution: ReferenceResolution::Unresolved,
                            });
                            findings.push(make_finding(
                                DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING,
                                ReferenceRequiredness::Required,
                                format!("Player transfer target map '{target_map_path}' does not exist on disk"),
                                vec![cmd_node_id.clone()],
                            ));
                        }
                    }
                } else {
                    // Variable transfer: appointment_method == 1
                    let dyn_id = "unresolved_dynamic_reference:variable_transfer".to_string();
                    if !graph.nodes.iter().any(|n| n.id == dyn_id) {
                        graph.nodes.push(AssetNode {
                            id: dyn_id.clone(),
                            kind: AssetKind::UnresolvedDynamicReference,
                            logical_path: None,
                        });
                    }
                    graph.edges.push(ReferenceEdge {
                        source: cmd_node_id.clone(),
                        target: dyn_id,
                        relation: "variable_transfer".into(),
                        requiredness: ReferenceRequiredness::Optional,
                        evidence: EvidenceTrail::through(EvidenceStage::ReferenceFound),
                        resolution: ReferenceResolution::Unresolved,
                    });
                }
            }
            // 212: Show Animation
            212 => {
                let anim_id = params.get(1).and_then(|v| v.as_i32()).unwrap_or(0);
                if anim_id > 0 {
                    let anim_target = format!("database_entity:Animation:{anim_id}");
                    if !graph.nodes.iter().any(|n| n.id == anim_target) {
                        graph.nodes.push(AssetNode {
                            id: anim_target.clone(),
                            kind: AssetKind::DatabaseEntity,
                            logical_path: None,
                        });
                    }
                    graph.edges.push(ReferenceEdge {
                        source: cmd_node_id.clone(),
                        target: anim_target,
                        relation: "animation".into(),
                        requiredness: ReferenceRequiredness::Required,
                        evidence: EvidenceTrail::through(EvidenceStage::ReferenceFound),
                        resolution: ReferenceResolution::Resolved,
                    });
                }
            }
            // 231: Show Picture
            231 => {
                if let Some(pic_name) = params.get(1).and_then(|v| v.as_str()) {
                    if !pic_name.is_empty() {
                        resolve_and_record(
                            resolver,
                            graph,
                            findings,
                            &cmd_node_id,
                            &format!("Graphics/Pictures/{pic_name}"),
                            IMAGE_EXTENSIONS,
                            AssetKind::Image,
                            ReferenceRequiredness::Required,
                        );
                    }
                }
            }
            // 241: Play BGM
            241 => {
                if let Some(audio) = params.first() {
                    audit_audio_val(audio, "Audio/BGM", &cmd_node_id, resolver, graph, findings);
                }
            }
            // 245: Play BGS
            245 => {
                if let Some(audio) = params.first() {
                    audit_audio_val(audio, "Audio/BGS", &cmd_node_id, resolver, graph, findings);
                }
            }
            // 249: Play ME
            249 => {
                if let Some(audio) = params.first() {
                    audit_audio_val(audio, "Audio/ME", &cmd_node_id, resolver, graph, findings);
                }
            }
            // 250: Play SE
            250 => {
                if let Some(audio) = params.first() {
                    audit_audio_val(audio, "Audio/SE", &cmd_node_id, resolver, graph, findings);
                }
            }
            // 322: Change Actor Graphic
            322 => match gen {
                RgssGeneration::Xp => {
                    if let Some(char_name) = params.get(1).and_then(|v| v.as_str()) {
                        if !char_name.is_empty() {
                            resolve_and_record(
                                resolver,
                                graph,
                                findings,
                                &cmd_node_id,
                                &format!("Graphics/Characters/{char_name}"),
                                IMAGE_EXTENSIONS,
                                AssetKind::Image,
                                ReferenceRequiredness::Required,
                            );
                        }
                    }
                    if let Some(battler_name) = params.get(3).and_then(|v| v.as_str()) {
                        if !battler_name.is_empty() {
                            resolve_and_record(
                                resolver,
                                graph,
                                findings,
                                &cmd_node_id,
                                &format!("Graphics/Battlers/{battler_name}"),
                                IMAGE_EXTENSIONS,
                                AssetKind::Image,
                                ReferenceRequiredness::Required,
                            );
                        }
                    }
                }
                RgssGeneration::Vx | RgssGeneration::VxAce => {
                    if let Some(char_name) = params.get(1).and_then(|v| v.as_str()) {
                        if !char_name.is_empty() {
                            resolve_and_record(
                                resolver,
                                graph,
                                findings,
                                &cmd_node_id,
                                &format!("Graphics/Characters/{char_name}"),
                                IMAGE_EXTENSIONS,
                                AssetKind::Image,
                                ReferenceRequiredness::Required,
                            );
                        }
                    }
                    if let Some(face_name) = params.get(3).and_then(|v| v.as_str()) {
                        if !face_name.is_empty() {
                            resolve_and_record(
                                resolver,
                                graph,
                                findings,
                                &cmd_node_id,
                                &format!("Graphics/Faces/{face_name}"),
                                IMAGE_EXTENSIONS,
                                AssetKind::Image,
                                ReferenceRequiredness::Required,
                            );
                        }
                    }
                }
            },
            _ => {}
        }
    }
}
