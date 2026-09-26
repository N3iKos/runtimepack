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
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::OnceLock;

fn make_finding(
    code: &'static str,
    requiredness: ReferenceRequiredness,
    message: String,
    source_node_ids: Vec<String>,
    finding_counter: &mut usize,
) -> AuditFinding {
    *finding_counter += 1;
    let (severity, disposition) = audit_finding_classification(code, requiredness, false)
        .unwrap_or((FindingSeverity::Warning, FindingDisposition::Warning));
    AuditFinding {
        id: format!("{code}-{finding_counter}"),
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

#[derive(Debug, PartialEq)]
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

    fn remaining_bytes(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
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
        if (5..=127).contains(&b) {
            return Ok((b - 5) as i32);
        }
        if (-128..=-5).contains(&b) {
            return Ok((b + 5) as i32);
        }
        if (1..=4).contains(&b) {
            let count = b as usize;
            let mut val = 0i64;
            for i in 0..count {
                let byte = self.read_byte()?;
                val |= (byte as i64) << (8 * i);
            }
            return Ok(val as i32);
        }
        if (-4..=-1).contains(&b) {
            let count = (-b) as usize;
            let mut val = -1i64;
            for i in 0..count {
                let byte = self.read_byte()?;
                val = (val & !(0xff << (8 * i))) | ((byte as i64) << (8 * i));
            }
            return Ok(val as i32);
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
                let rem = self.remaining_bytes();
                if (len as usize) > rem {
                    return Err(MarshalError::Corrupt(format!(
                        "symbol length {len} exceeds remaining bytes {rem}"
                    )));
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
                let rem = self.remaining_bytes();
                if (len as usize) > rem {
                    return Err(MarshalError::Corrupt(format!(
                        "symbol length {len} exceeds remaining bytes {rem}"
                    )));
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
                let rem = self.remaining_bytes();
                if (len as usize) > rem {
                    return Err(MarshalError::Corrupt(format!(
                        "string length {len} exceeds remaining bytes {rem}"
                    )));
                }
                let bytes = self.read_bytes(len as usize)?.to_vec();
                let val = MarshalValue::String(bytes);
                self.objects[slot] = val.clone();
                Ok(val)
            }
            b'I' => {
                let slot = self.objects.len();
                let target = self.parse_value()?;
                let ivar_count = self.read_long()?;
                if ivar_count < 0 {
                    return Err(MarshalError::Corrupt("negative ivar count".into()));
                }
                let rem = self.remaining_bytes();
                if (ivar_count as usize) > rem {
                    return Err(MarshalError::Corrupt(format!(
                        "ivar count {ivar_count} exceeds remaining bytes {rem}"
                    )));
                }
                let mut ivars = Vec::with_capacity((ivar_count as usize).min(rem));
                for _ in 0..ivar_count {
                    let sym = self.read_symbol()?;
                    let val = self.parse_value()?;
                    ivars.push((sym, val));
                }
                let res = MarshalValue::InstanceVars {
                    target: Box::new(target),
                    ivars,
                };
                if slot < self.objects.len() {
                    self.objects[slot] = res.clone();
                } else {
                    self.objects.push(res.clone());
                }
                Ok(res)
            }
            b'[' => {
                let slot = self.objects.len();
                self.objects.push(MarshalValue::Nil);
                let len = self.read_long()?;
                if len < 0 {
                    return Err(MarshalError::Corrupt("negative array length".into()));
                }
                let rem = self.remaining_bytes();
                if (len as usize) > rem {
                    return Err(MarshalError::Corrupt(format!(
                        "array length {len} exceeds remaining bytes {rem}"
                    )));
                }
                let mut items = Vec::with_capacity((len as usize).min(rem));
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
                let rem = self.remaining_bytes();
                if (len as usize) > rem {
                    return Err(MarshalError::Corrupt(format!(
                        "hash length {len} exceeds remaining bytes {rem}"
                    )));
                }
                let mut pairs = Vec::with_capacity((len as usize).min(rem));
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
                let rem = self.remaining_bytes();
                if (ivar_count as usize) > rem {
                    return Err(MarshalError::Corrupt(format!(
                        "object ivar count {ivar_count} exceeds remaining bytes {rem}"
                    )));
                }
                let mut ivars = Vec::with_capacity((ivar_count as usize).min(rem));
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
                let rem = self.remaining_bytes();
                if (len as usize) > rem {
                    return Err(MarshalError::Corrupt(format!(
                        "user data length {len} exceeds remaining bytes {rem}"
                    )));
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
                let rem = self.remaining_bytes();
                if (len as usize) > rem {
                    return Err(MarshalError::Corrupt(format!(
                        "float length {len} exceeds remaining bytes {rem}"
                    )));
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
                let rem = self.remaining_bytes();
                if (len as usize * 2) > rem {
                    return Err(MarshalError::Corrupt(format!(
                        "bignum length {len} exceeds remaining bytes {rem}"
                    )));
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
        let mut finding_counter = 0usize;
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
                        &mut finding_counter,
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
        let mut node_ids = HashSet::new();
        let mut coverage = AuditCoverage::new();
        let resolver = AssetResolver::new(source);

        // Check limits: max_files, max_total_bytes, max_single_file_bytes
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
                &mut finding_counter,
            ));
        }

        for file in source.files.values() {
            if file.size_bytes > options.limits.max_single_file_bytes {
                findings.push(make_finding(
                    DiagnosticCode::CMP_AUD_LIMIT_REACHED,
                    ReferenceRequiredness::Required,
                    format!(
                        "File '{}' size {} bytes exceeds max_single_file_bytes limit {}",
                        file.logical_path, file.size_bytes, options.limits.max_single_file_bytes
                    ),
                    vec![file.logical_path.clone()],
                    &mut finding_counter,
                ));
            }
        }

        // Declare covered reference classes in standard mode
        coverage.add_covered("system");
        coverage.add_covered("maps");
        coverage.add_covered("events");
        coverage.add_covered("database");
        coverage.add_covered("fonts");

        let ext = gen.data_extension();

        // Check encrypted archive
        let has_encrypted_archive = source.files.values().any(|f| {
            f.logical_path
                .to_ascii_lowercase()
                .ends_with(gen.encrypted_extension())
        });

        // Required database files check (Critical 2)
        let required_db_files: &[&str] = match gen {
            RgssGeneration::Xp => &[
                "System.rxdata",
                "Actors.rxdata",
                "Classes.rxdata",
                "Skills.rxdata",
                "Items.rxdata",
                "Weapons.rxdata",
                "Armors.rxdata",
                "Enemies.rxdata",
                "Troops.rxdata",
                "States.rxdata",
                "Animations.rxdata",
                "Tilesets.rxdata",
                "CommonEvents.rxdata",
            ],
            RgssGeneration::Vx => &[
                "System.rvdata",
                "Actors.rvdata",
                "Classes.rvdata",
                "Skills.rvdata",
                "Items.rvdata",
                "Weapons.rvdata",
                "Armors.rvdata",
                "Enemies.rvdata",
                "Troops.rvdata",
                "States.rvdata",
                "Animations.rvdata",
                "CommonEvents.rvdata",
            ],
            RgssGeneration::VxAce => &[
                "System.rvdata2",
                "Actors.rvdata2",
                "Classes.rvdata2",
                "Skills.rvdata2",
                "Items.rvdata2",
                "Weapons.rvdata2",
                "Armors.rvdata2",
                "Enemies.rvdata2",
                "Troops.rvdata2",
                "States.rvdata2",
                "Animations.rvdata2",
                "Tilesets.rvdata2",
                "CommonEvents.rvdata2",
            ],
        };

        if !has_encrypted_archive {
            for db_file in required_db_files {
                let db_path = format!("Data/{db_file}");
                if source.get_file(&db_path).is_none() {
                    let source_id = format!("database_file:{db_path}");
                    findings.push(make_finding(
                        DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING,
                        ReferenceRequiredness::Required,
                        format!("Required database file '{db_path}' is missing on disk"),
                        vec![source_id.clone()],
                        &mut finding_counter,
                    ));
                    if node_ids.insert(source_id.clone()) {
                        graph.nodes.push(AssetNode {
                            id: source_id,
                            kind: AssetKind::DatabaseEntity,
                            logical_path: Some(db_path),
                        });
                    }
                }
            }
        }

        // 1. Audit System data
        let system_path = format!("Data/System.{ext}");
        audit_system_file(
            &system_path,
            gen,
            source,
            &resolver,
            &mut graph,
            &mut node_ids,
            &mut findings,
            &mut finding_counter,
        );

        // 2. Audit Database files
        audit_database_files(
            gen,
            source,
            &resolver,
            &mut graph,
            &mut node_ids,
            &mut findings,
            &mut finding_counter,
        );

        // 3. Audit Map files
        audit_map_files(
            gen,
            source,
            &resolver,
            &mut graph,
            &mut node_ids,
            &mut findings,
            &mut finding_counter,
        );

        // 4. Audit Font & CJK requirements
        audit_font_requirements(
            gen,
            source,
            &mut coverage,
            &mut graph,
            &mut node_ids,
            &mut findings,
            &mut finding_counter,
        );

        // Archive header validation for .rgss3a
        for f in source.files.values() {
            let lower = f.logical_path.to_ascii_lowercase();
            if lower.ends_with(".rgss3a") {
                let archive_node_id = format!("archive:{}", f.logical_path);
                if node_ids.insert(archive_node_id.clone()) {
                    graph.nodes.push(AssetNode {
                        id: archive_node_id.clone(),
                        kind: AssetKind::ArchiveEntry,
                        logical_path: Some(f.logical_path.clone()),
                    });
                }

                let full_path = f.real_path(&source.root_path);
                let header_check = match std::fs::File::open(&full_path) {
                    Ok(mut file) => {
                        use std::io::Read;
                        let mut header = [0u8; 8];
                        match file.read(&mut header) {
                            Ok(n) if n >= 8 && header.starts_with(b"RGSSAD\0\x03") => Ok(()),
                            Ok(n) if n >= 7 && header.starts_with(b"RGSSAD\x03") => Ok(()),
                            Ok(n) if n < 7 => Err(format!(
                                "Encrypted archive '{}' is truncated (header size {} < 8 bytes)",
                                f.logical_path, n
                            )),
                            Ok(_) => Err(format!(
                                "Encrypted archive '{}' has invalid header magic or version (expected RGSSAD\\0\\x03)",
                                f.logical_path
                            )),
                            Err(e) => Err(format!(
                                "Failed to read encrypted archive '{}': {e}",
                                f.logical_path
                            )),
                        }
                    }
                    Err(e) => Err(format!(
                        "Failed to open encrypted archive '{}': {e}",
                        f.logical_path
                    )),
                };

                if let Err(err_msg) = header_check {
                    findings.push(make_finding(
                        DiagnosticCode::CMP_AUD_MALFORMED_RESOURCE,
                        ReferenceRequiredness::Required,
                        err_msg,
                        vec![archive_node_id],
                        &mut finding_counter,
                    ));
                }
            }
        }

        // 4. Standard vs Deep mode checks
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
                    &mut finding_counter,
                ));
            } else if !source.has_file(&system_path) {
                // In standard mode, if Data/System does not exist unpacked because it is in an encrypted archive
                coverage.add_unsupported("encrypted_archive", ReferenceRequiredness::Required);
                findings.push(make_finding(
                    DiagnosticCode::CMP_AUD_DYNAMIC_REFERENCE_UNRESOLVED,
                    ReferenceRequiredness::Required,
                    format!("Game content is stored in encrypted archive '{archive_path}' without unpacked data files"),
                    vec![archive_path],
                    &mut finding_counter,
                ));
            }
        }

        if options.mode == AuditMode::Deep {
            let scripts_path = format!("Data/Scripts.{ext}");
            if let Some(indexed_file) = source.get_file(&scripts_path) {
                let full_path = indexed_file.real_path(&source.root_path);
                match std::fs::read(&full_path) {
                    Ok(bytes) => match parse_marshal(&bytes) {
                        Ok(_) => {
                            coverage.add_unsupported("scripts", ReferenceRequiredness::Required);
                            findings.push(make_finding(
                                DiagnosticCode::CMP_AUD_DYNAMIC_REFERENCE_UNRESOLVED,
                                ReferenceRequiredness::Required,
                                format!("RGSS script archive '{scripts_path}' contains dynamic Ruby code that cannot be fully verified statically"),
                                vec![scripts_path],
                                &mut finding_counter,
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
                                &mut finding_counter,
                            ));
                        }
                    },
                    Err(e) => {
                        findings.push(make_finding(
                            DiagnosticCode::CMP_AUD_MALFORMED_RESOURCE,
                            ReferenceRequiredness::Required,
                            format!("Failed to read script archive '{scripts_path}': {e}"),
                            vec![scripts_path],
                            &mut finding_counter,
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
    finding_counter: &mut usize,
) -> Option<MarshalValue> {
    let indexed_file = source.get_file(logical_path)?;
    let full_path = indexed_file.real_path(&source.root_path);
    let bytes = match std::fs::read(&full_path) {
        Ok(b) => b,
        Err(e) => {
            findings.push(make_finding(
                DiagnosticCode::CMP_AUD_MALFORMED_RESOURCE,
                ReferenceRequiredness::Required,
                format!("Failed to read data file '{logical_path}': {e}"),
                vec![logical_path.to_string()],
                finding_counter,
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
                finding_counter,
            ));
            None
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_and_record(
    resolver: &AssetResolver,
    graph: &mut DependencyGraph,
    node_ids: &mut HashSet<String>,
    findings: &mut Vec<AuditFinding>,
    source_node_id: &str,
    target_base_path: &str,
    extensions: &[&str],
    kind: AssetKind,
    requiredness: ReferenceRequiredness,
    finding_counter: &mut usize,
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
                finding_counter,
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
                finding_counter,
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
                finding_counter,
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
                finding_counter,
            ));
            (
                format!("{}:{target_base_path}", kind.as_str()),
                ReferenceResolution::Unresolved,
                None,
            )
        }
    };

    if node_ids.insert(target_id.clone()) {
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

#[allow(clippy::too_many_arguments)]
fn audit_audio_val(
    audio: &MarshalValue,
    folder: &str,
    source_node: &str,
    resolver: &AssetResolver,
    graph: &mut DependencyGraph,
    node_ids: &mut HashSet<String>,
    findings: &mut Vec<AuditFinding>,
    finding_counter: &mut usize,
) {
    if let Some(name) = audio.get_ivar_str("@name").or_else(|| audio.as_str()) {
        if !name.is_empty() {
            let base = format!("{folder}/{name}");
            resolve_and_record(
                resolver,
                graph,
                node_ids,
                findings,
                source_node,
                &base,
                AUDIO_EXTENSIONS,
                AssetKind::Audio,
                ReferenceRequiredness::Required,
                finding_counter,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn audit_system_file(
    logical_path: &str,
    gen: RgssGeneration,
    source: &GameSourceIndex,
    resolver: &AssetResolver,
    graph: &mut DependencyGraph,
    node_ids: &mut HashSet<String>,
    findings: &mut Vec<AuditFinding>,
    finding_counter: &mut usize,
) {
    let sys = match read_and_parse_data_file(logical_path, source, findings, finding_counter) {
        Some(s) => s,
        None => return,
    };
    let sys_node_id = "Database:System".to_string();
    if node_ids.insert(sys_node_id.clone()) {
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
                            node_ids,
                            findings,
                            &sys_node_id,
                            &format!("{folder}/{name}"),
                            IMAGE_EXTENSIONS,
                            AssetKind::Image,
                            ReferenceRequiredness::Required,
                            finding_counter,
                        );
                    }
                }
            }

            for field in ["@title_bgm", "@battle_bgm"] {
                if let Some(audio) = sys.get_ivar(field) {
                    audit_audio_val(
                        audio,
                        "Audio/BGM",
                        &sys_node_id,
                        resolver,
                        graph,
                        node_ids,
                        findings,
                        finding_counter,
                    );
                }
            }
            for field in ["@battle_end_me", "@gameover_me"] {
                if let Some(audio) = sys.get_ivar(field) {
                    audit_audio_val(
                        audio,
                        "Audio/ME",
                        &sys_node_id,
                        resolver,
                        graph,
                        node_ids,
                        findings,
                        finding_counter,
                    );
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
                    audit_audio_val(
                        audio,
                        "Audio/SE",
                        &sys_node_id,
                        resolver,
                        graph,
                        node_ids,
                        findings,
                        finding_counter,
                    );
                }
            }
        }
        RgssGeneration::Vx => {
            if let Some(name) = sys.get_ivar_str("@gameover_name") {
                if !name.is_empty() {
                    resolve_and_record(
                        resolver,
                        graph,
                        node_ids,
                        findings,
                        &sys_node_id,
                        &format!("Graphics/System/{name}"),
                        IMAGE_EXTENSIONS,
                        AssetKind::Image,
                        ReferenceRequiredness::Required,
                        finding_counter,
                    );
                }
            }
            for field in ["@title_bgm", "@battle_bgm"] {
                if let Some(audio) = sys.get_ivar(field) {
                    audit_audio_val(
                        audio,
                        "Audio/BGM",
                        &sys_node_id,
                        resolver,
                        graph,
                        node_ids,
                        findings,
                        finding_counter,
                    );
                }
            }
            for field in ["@battle_end_me", "@gameover_me"] {
                if let Some(audio) = sys.get_ivar(field) {
                    audit_audio_val(
                        audio,
                        "Audio/ME",
                        &sys_node_id,
                        resolver,
                        graph,
                        node_ids,
                        findings,
                        finding_counter,
                    );
                }
            }
            if let Some(sounds) = sys.get_ivar("@sounds").and_then(|v| v.as_array()) {
                for sound in sounds {
                    audit_audio_val(
                        sound,
                        "Audio/SE",
                        &sys_node_id,
                        resolver,
                        graph,
                        node_ids,
                        findings,
                        finding_counter,
                    );
                }
            }
            for v_name in ["@boat", "@ship", "@airship"] {
                if let Some(vehicle) = sys.get_ivar(v_name) {
                    if let Some(c_name) = vehicle.get_ivar_str("@character_name") {
                        if !c_name.is_empty() {
                            resolve_and_record(
                                resolver,
                                graph,
                                node_ids,
                                findings,
                                &sys_node_id,
                                &format!("Graphics/Characters/{c_name}"),
                                IMAGE_EXTENSIONS,
                                AssetKind::Image,
                                ReferenceRequiredness::Required,
                                finding_counter,
                            );
                        }
                    }
                    if let Some(bgm) = vehicle.get_ivar("@bgm") {
                        audit_audio_val(
                            bgm,
                            "Audio/BGM",
                            &sys_node_id,
                            resolver,
                            graph,
                            node_ids,
                            findings,
                            finding_counter,
                        );
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
                            node_ids,
                            findings,
                            &sys_node_id,
                            &format!("{folder}/{name}"),
                            IMAGE_EXTENSIONS,
                            AssetKind::Image,
                            ReferenceRequiredness::Required,
                            finding_counter,
                        );
                    }
                }
            }
            for field in ["@title_bgm", "@battle_bgm"] {
                if let Some(audio) = sys.get_ivar(field) {
                    audit_audio_val(
                        audio,
                        "Audio/BGM",
                        &sys_node_id,
                        resolver,
                        graph,
                        node_ids,
                        findings,
                        finding_counter,
                    );
                }
            }
            for field in ["@battle_end_me", "@gameover_me"] {
                if let Some(audio) = sys.get_ivar(field) {
                    audit_audio_val(
                        audio,
                        "Audio/ME",
                        &sys_node_id,
                        resolver,
                        graph,
                        node_ids,
                        findings,
                        finding_counter,
                    );
                }
            }
            if let Some(sounds) = sys.get_ivar("@sounds").and_then(|v| v.as_array()) {
                for sound in sounds {
                    audit_audio_val(
                        sound,
                        "Audio/SE",
                        &sys_node_id,
                        resolver,
                        graph,
                        node_ids,
                        findings,
                        finding_counter,
                    );
                }
            }
            for v_name in ["@boat", "@ship", "@airship"] {
                if let Some(vehicle) = sys.get_ivar(v_name) {
                    if let Some(c_name) = vehicle.get_ivar_str("@character_name") {
                        if !c_name.is_empty() {
                            resolve_and_record(
                                resolver,
                                graph,
                                node_ids,
                                findings,
                                &sys_node_id,
                                &format!("Graphics/Characters/{c_name}"),
                                IMAGE_EXTENSIONS,
                                AssetKind::Image,
                                ReferenceRequiredness::Required,
                                finding_counter,
                            );
                        }
                    }
                    if let Some(bgm) = vehicle.get_ivar("@bgm") {
                        audit_audio_val(
                            bgm,
                            "Audio/BGM",
                            &sys_node_id,
                            resolver,
                            graph,
                            node_ids,
                            findings,
                            finding_counter,
                        );
                    }
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn audit_database_files(
    gen: RgssGeneration,
    source: &GameSourceIndex,
    resolver: &AssetResolver,
    graph: &mut DependencyGraph,
    node_ids: &mut HashSet<String>,
    findings: &mut Vec<AuditFinding>,
    finding_counter: &mut usize,
) {
    let ext = gen.data_extension();

    // Actors
    let actors_path = format!("Data/Actors.{ext}");
    if let Some(val) = read_and_parse_data_file(&actors_path, source, findings, finding_counter) {
        if let Some(actors) = val.as_array() {
            for actor in actors {
                let id = actor.get_ivar_i32("@id").unwrap_or(0);
                if id == 0 {
                    continue;
                }
                let node_id = format!("Database:Actors:{id}");
                if node_ids.insert(node_id.clone()) {
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
                            node_ids,
                            findings,
                            &node_id,
                            &format!("Graphics/Characters/{c_name}"),
                            IMAGE_EXTENSIONS,
                            AssetKind::Image,
                            ReferenceRequiredness::Required,
                            finding_counter,
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
                                    node_ids,
                                    findings,
                                    &node_id,
                                    &format!("Graphics/Battlers/{b_name}"),
                                    IMAGE_EXTENSIONS,
                                    AssetKind::Image,
                                    ReferenceRequiredness::Required,
                                    finding_counter,
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
                                    node_ids,
                                    findings,
                                    &node_id,
                                    &format!("Graphics/Faces/{f_name}"),
                                    IMAGE_EXTENSIONS,
                                    AssetKind::Image,
                                    ReferenceRequiredness::Required,
                                    finding_counter,
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    // Classes
    let classes_path = format!("Data/Classes.{ext}");
    if let Some(val) = read_and_parse_data_file(&classes_path, source, findings, finding_counter) {
        if let Some(classes) = val.as_array() {
            for cls in classes {
                let id = cls.get_ivar_i32("@id").unwrap_or(0);
                if id == 0 {
                    continue;
                }
                let node_id = format!("Database:Classes:{id}");
                if node_ids.insert(node_id.clone()) {
                    graph.nodes.push(AssetNode {
                        id: node_id,
                        kind: AssetKind::DatabaseEntity,
                        logical_path: None,
                    });
                }
            }
        }
    }

    // Skills
    let skills_path = format!("Data/Skills.{ext}");
    if let Some(val) = read_and_parse_data_file(&skills_path, source, findings, finding_counter) {
        if let Some(skills) = val.as_array() {
            for skill in skills {
                let id = skill.get_ivar_i32("@id").unwrap_or(0);
                if id == 0 {
                    continue;
                }
                let node_id = format!("Database:Skills:{id}");
                if node_ids.insert(node_id.clone()) {
                    graph.nodes.push(AssetNode {
                        id: node_id.clone(),
                        kind: AssetKind::DatabaseEntity,
                        logical_path: None,
                    });
                }
                match gen {
                    RgssGeneration::Xp => {
                        if let Some(icon) = skill.get_ivar_str("@icon_name") {
                            if !icon.is_empty() {
                                resolve_and_record(
                                    resolver,
                                    graph,
                                    node_ids,
                                    findings,
                                    &node_id,
                                    &format!("Graphics/Icons/{icon}"),
                                    IMAGE_EXTENSIONS,
                                    AssetKind::Image,
                                    ReferenceRequiredness::Required,
                                    finding_counter,
                                );
                            }
                        }
                        if let Some(se) = skill.get_ivar("@sound_effect") {
                            audit_audio_val(
                                se,
                                "Audio/SE",
                                &node_id,
                                resolver,
                                graph,
                                node_ids,
                                findings,
                                finding_counter,
                            );
                        }
                    }
                    RgssGeneration::Vx | RgssGeneration::VxAce => {
                        if skill.get_ivar_i32("@icon_index").unwrap_or(0) > 0 {
                            resolve_and_record(
                                resolver,
                                graph,
                                node_ids,
                                findings,
                                &node_id,
                                "Graphics/System/IconSet",
                                IMAGE_EXTENSIONS,
                                AssetKind::Image,
                                ReferenceRequiredness::Required,
                                finding_counter,
                            );
                        }
                    }
                }
            }
        }
    }

    // Items
    let items_path = format!("Data/Items.{ext}");
    if let Some(val) = read_and_parse_data_file(&items_path, source, findings, finding_counter) {
        if let Some(items) = val.as_array() {
            for item in items {
                let id = item.get_ivar_i32("@id").unwrap_or(0);
                if id == 0 {
                    continue;
                }
                let node_id = format!("Database:Items:{id}");
                if node_ids.insert(node_id.clone()) {
                    graph.nodes.push(AssetNode {
                        id: node_id.clone(),
                        kind: AssetKind::DatabaseEntity,
                        logical_path: None,
                    });
                }
                match gen {
                    RgssGeneration::Xp => {
                        if let Some(icon) = item.get_ivar_str("@icon_name") {
                            if !icon.is_empty() {
                                resolve_and_record(
                                    resolver,
                                    graph,
                                    node_ids,
                                    findings,
                                    &node_id,
                                    &format!("Graphics/Icons/{icon}"),
                                    IMAGE_EXTENSIONS,
                                    AssetKind::Image,
                                    ReferenceRequiredness::Required,
                                    finding_counter,
                                );
                            }
                        }
                        if let Some(menu_se) = item.get_ivar_str("@menu_se_name") {
                            if !menu_se.is_empty() {
                                resolve_and_record(
                                    resolver,
                                    graph,
                                    node_ids,
                                    findings,
                                    &node_id,
                                    &format!("Audio/SE/{menu_se}"),
                                    AUDIO_EXTENSIONS,
                                    AssetKind::Audio,
                                    ReferenceRequiredness::Required,
                                    finding_counter,
                                );
                            }
                        }
                    }
                    RgssGeneration::Vx | RgssGeneration::VxAce => {
                        if item.get_ivar_i32("@icon_index").unwrap_or(0) > 0 {
                            resolve_and_record(
                                resolver,
                                graph,
                                node_ids,
                                findings,
                                &node_id,
                                "Graphics/System/IconSet",
                                IMAGE_EXTENSIONS,
                                AssetKind::Image,
                                ReferenceRequiredness::Required,
                                finding_counter,
                            );
                        }
                    }
                }
            }
        }
    }

    // Weapons
    let weapons_path = format!("Data/Weapons.{ext}");
    if let Some(val) = read_and_parse_data_file(&weapons_path, source, findings, finding_counter) {
        if let Some(weapons) = val.as_array() {
            for weapon in weapons {
                let id = weapon.get_ivar_i32("@id").unwrap_or(0);
                if id == 0 {
                    continue;
                }
                let node_id = format!("Database:Weapons:{id}");
                if node_ids.insert(node_id.clone()) {
                    graph.nodes.push(AssetNode {
                        id: node_id.clone(),
                        kind: AssetKind::DatabaseEntity,
                        logical_path: None,
                    });
                }
                match gen {
                    RgssGeneration::Xp => {
                        if let Some(icon) = weapon.get_ivar_str("@icon_name") {
                            if !icon.is_empty() {
                                resolve_and_record(
                                    resolver,
                                    graph,
                                    node_ids,
                                    findings,
                                    &node_id,
                                    &format!("Graphics/Icons/{icon}"),
                                    IMAGE_EXTENSIONS,
                                    AssetKind::Image,
                                    ReferenceRequiredness::Required,
                                    finding_counter,
                                );
                            }
                        }
                    }
                    RgssGeneration::Vx | RgssGeneration::VxAce => {
                        if weapon.get_ivar_i32("@icon_index").unwrap_or(0) > 0 {
                            resolve_and_record(
                                resolver,
                                graph,
                                node_ids,
                                findings,
                                &node_id,
                                "Graphics/System/IconSet",
                                IMAGE_EXTENSIONS,
                                AssetKind::Image,
                                ReferenceRequiredness::Required,
                                finding_counter,
                            );
                        }
                    }
                }
            }
        }
    }

    // Armors
    let armors_path = format!("Data/Armors.{ext}");
    if let Some(val) = read_and_parse_data_file(&armors_path, source, findings, finding_counter) {
        if let Some(armors) = val.as_array() {
            for armor in armors {
                let id = armor.get_ivar_i32("@id").unwrap_or(0);
                if id == 0 {
                    continue;
                }
                let node_id = format!("Database:Armors:{id}");
                if node_ids.insert(node_id.clone()) {
                    graph.nodes.push(AssetNode {
                        id: node_id.clone(),
                        kind: AssetKind::DatabaseEntity,
                        logical_path: None,
                    });
                }
                match gen {
                    RgssGeneration::Xp => {
                        if let Some(icon) = armor.get_ivar_str("@icon_name") {
                            if !icon.is_empty() {
                                resolve_and_record(
                                    resolver,
                                    graph,
                                    node_ids,
                                    findings,
                                    &node_id,
                                    &format!("Graphics/Icons/{icon}"),
                                    IMAGE_EXTENSIONS,
                                    AssetKind::Image,
                                    ReferenceRequiredness::Required,
                                    finding_counter,
                                );
                            }
                        }
                    }
                    RgssGeneration::Vx | RgssGeneration::VxAce => {
                        if armor.get_ivar_i32("@icon_index").unwrap_or(0) > 0 {
                            resolve_and_record(
                                resolver,
                                graph,
                                node_ids,
                                findings,
                                &node_id,
                                "Graphics/System/IconSet",
                                IMAGE_EXTENSIONS,
                                AssetKind::Image,
                                ReferenceRequiredness::Required,
                                finding_counter,
                            );
                        }
                    }
                }
            }
        }
    }

    // States
    let states_path = format!("Data/States.{ext}");
    if let Some(val) = read_and_parse_data_file(&states_path, source, findings, finding_counter) {
        if let Some(states) = val.as_array() {
            for state in states {
                let id = state.get_ivar_i32("@id").unwrap_or(0);
                if id == 0 {
                    continue;
                }
                let node_id = format!("Database:States:{id}");
                if node_ids.insert(node_id.clone()) {
                    graph.nodes.push(AssetNode {
                        id: node_id.clone(),
                        kind: AssetKind::DatabaseEntity,
                        logical_path: None,
                    });
                }
                if matches!(gen, RgssGeneration::Vx | RgssGeneration::VxAce)
                    && state.get_ivar_i32("@icon_index").unwrap_or(0) > 0
                {
                    resolve_and_record(
                        resolver,
                        graph,
                        node_ids,
                        findings,
                        &node_id,
                        "Graphics/System/IconSet",
                        IMAGE_EXTENSIONS,
                        AssetKind::Image,
                        ReferenceRequiredness::Required,
                        finding_counter,
                    );
                }
            }
        }
    }

    // Enemies
    let enemies_path = format!("Data/Enemies.{ext}");
    if let Some(val) = read_and_parse_data_file(&enemies_path, source, findings, finding_counter) {
        if let Some(enemies) = val.as_array() {
            for enemy in enemies {
                let id = enemy.get_ivar_i32("@id").unwrap_or(0);
                if id == 0 {
                    continue;
                }
                let node_id = format!("Database:Enemies:{id}");
                if node_ids.insert(node_id.clone()) {
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
                            node_ids,
                            findings,
                            &node_id,
                            &format!("Graphics/Battlers/{b_name}"),
                            IMAGE_EXTENSIONS,
                            AssetKind::Image,
                            ReferenceRequiredness::Required,
                            finding_counter,
                        );
                    }
                }
            }
        }
    }

    // Tilesets
    let tilesets_path = format!("Data/Tilesets.{ext}");
    if let Some(val) = read_and_parse_data_file(&tilesets_path, source, findings, finding_counter) {
        if let Some(tilesets) = val.as_array() {
            for ts in tilesets {
                let id = ts.get_ivar_i32("@id").unwrap_or(0);
                if id == 0 {
                    continue;
                }
                let node_id = format!("Database:Tilesets:{id}");
                if node_ids.insert(node_id.clone()) {
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
                                    node_ids,
                                    findings,
                                    &node_id,
                                    &format!("Graphics/Tilesets/{ts_name}"),
                                    IMAGE_EXTENSIONS,
                                    AssetKind::Image,
                                    ReferenceRequiredness::Required,
                                    finding_counter,
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
                                            node_ids,
                                            findings,
                                            &node_id,
                                            &format!("Graphics/Autotiles/{at_name}"),
                                            IMAGE_EXTENSIONS,
                                            AssetKind::Image,
                                            ReferenceRequiredness::Required,
                                            finding_counter,
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
                                        node_ids,
                                        findings,
                                        &node_id,
                                        &format!("{folder}/{name}"),
                                        IMAGE_EXTENSIONS,
                                        AssetKind::Image,
                                        ReferenceRequiredness::Required,
                                        finding_counter,
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
                                            node_ids,
                                            findings,
                                            &node_id,
                                            &format!("Graphics/Tilesets/{name}"),
                                            IMAGE_EXTENSIONS,
                                            AssetKind::Image,
                                            ReferenceRequiredness::Required,
                                            finding_counter,
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
    if let Some(val) = read_and_parse_data_file(&anims_path, source, findings, finding_counter) {
        if let Some(anims) = val.as_array() {
            for anim in anims {
                let id = anim.get_ivar_i32("@id").unwrap_or(0);
                if id == 0 {
                    continue;
                }
                let node_id = format!("database_entity:Animation:{id}");
                if node_ids.insert(node_id.clone()) {
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
                                    node_ids,
                                    findings,
                                    &node_id,
                                    &format!("Graphics/Animations/{name}"),
                                    IMAGE_EXTENSIONS,
                                    AssetKind::Image,
                                    ReferenceRequiredness::Required,
                                    finding_counter,
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
                                        node_ids,
                                        findings,
                                        &node_id,
                                        &format!("Graphics/Animations/{name}"),
                                        IMAGE_EXTENSIONS,
                                        AssetKind::Image,
                                        ReferenceRequiredness::Required,
                                        finding_counter,
                                    );
                                }
                            }
                        }
                    }
                }
                if let Some(timings) = anim.get_ivar("@timings").and_then(|v| v.as_array()) {
                    for timing in timings {
                        if let Some(se) = timing.get_ivar("@se") {
                            audit_audio_val(
                                se,
                                "Audio/SE",
                                &node_id,
                                resolver,
                                graph,
                                node_ids,
                                findings,
                                finding_counter,
                            );
                        }
                    }
                }
            }
        }
    }

    // CommonEvents
    let ce_path = format!("Data/CommonEvents.{ext}");
    if let Some(val) = read_and_parse_data_file(&ce_path, source, findings, finding_counter) {
        if let Some(ces) = val.as_array() {
            for ce in ces {
                let id = ce.get_ivar_i32("@id").unwrap_or(0);
                if id == 0 {
                    continue;
                }
                let node_id = format!("Database:CommonEvents:{id}");
                if node_ids.insert(node_id.clone()) {
                    graph.nodes.push(AssetNode {
                        id: node_id.clone(),
                        kind: AssetKind::DatabaseEntity,
                        logical_path: None,
                    });
                }
                if let Some(list) = ce.get_ivar("@list").and_then(|v| v.as_array()) {
                    audit_event_commands(
                        list,
                        &node_id,
                        gen,
                        resolver,
                        graph,
                        node_ids,
                        findings,
                        finding_counter,
                    );
                }
            }
        }
    }

    // Troops
    let troops_path = format!("Data/Troops.{ext}");
    if let Some(val) = read_and_parse_data_file(&troops_path, source, findings, finding_counter) {
        if let Some(troops) = val.as_array() {
            for troop in troops {
                let id = troop.get_ivar_i32("@id").unwrap_or(0);
                if id == 0 {
                    continue;
                }
                let node_id = format!("Database:Troops:{id}");
                if node_ids.insert(node_id.clone()) {
                    graph.nodes.push(AssetNode {
                        id: node_id.clone(),
                        kind: AssetKind::DatabaseEntity,
                        logical_path: None,
                    });
                }
                if let Some(pages) = troop.get_ivar("@pages").and_then(|v| v.as_array()) {
                    for (page_idx, page) in pages.iter().enumerate() {
                        let page_node_id = format!("{node_id}:Page{}", page_idx + 1);
                        if node_ids.insert(page_node_id.clone()) {
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
                                node_ids,
                                findings,
                                finding_counter,
                            );
                        }
                    }
                }
            }
        }
    }

    // MapInfos (verify parsing)
    let mapinfos_path = format!("Data/MapInfos.{ext}");
    let _ = read_and_parse_data_file(&mapinfos_path, source, findings, finding_counter);
}

fn is_map_file(logical_path: &str, ext: &str) -> bool {
    let lower = logical_path.to_ascii_lowercase();
    let expected_suffix = format!(".{}", ext.to_ascii_lowercase());
    if !lower.starts_with("data/map") || !lower.ends_with(&expected_suffix) {
        return false;
    }
    let filename = logical_path.rsplit('/').next().unwrap_or(logical_path);
    let lower_filename = filename.to_ascii_lowercase();
    if let Some(rest) = lower_filename.strip_prefix("map") {
        if let Some(digits) = rest.strip_suffix(&expected_suffix) {
            return !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit());
        }
    }
    false
}

fn tile_id_to_tileset_slot(tile_id: u16) -> Option<usize> {
    match tile_id {
        0 => None,
        1..=255 => Some(5),     // B (Props, Furniture)
        256..=511 => Some(6),   // C
        512..=767 => Some(7),   // D
        768..=1023 => Some(8),  // E
        1536..=2047 => Some(4), // A5 (Normal Floor & Walls)
        2048..=2815 => Some(0), // A1 (Animation / Water)
        2816..=4351 => Some(1), // A2 (Ground / Floor)
        4352..=5887 => Some(2), // A3 (Buildings)
        5888..=7423 => Some(3), // A4 (Walls)
        _ => None,
    }
}

fn load_tilesets_map(source: &GameSourceIndex, ext: &str) -> HashMap<i32, Vec<String>> {
    let tilesets_path = format!("Data/Tilesets.{ext}");
    let indexed_file = match source.get_file(&tilesets_path) {
        Some(f) => f,
        None => return HashMap::new(),
    };
    let full_path = indexed_file.real_path(&source.root_path);
    let bytes = match std::fs::read(&full_path) {
        Ok(b) => b,
        Err(_) => return HashMap::new(),
    };
    let val = match parse_marshal(&bytes) {
        Ok(v) => v,
        Err(_) => return HashMap::new(),
    };
    let mut map = HashMap::new();
    if let Some(tilesets) = val.as_array() {
        for ts in tilesets {
            let id = ts.get_ivar_i32("@id").unwrap_or(0);
            if id == 0 {
                continue;
            }
            if let Some(names_arr) = ts.get_ivar("@tileset_names").and_then(|v| v.as_array()) {
                let names: Vec<String> = names_arr
                    .iter()
                    .map(|v| v.as_str().unwrap_or("").to_string())
                    .collect();
                map.insert(id, names);
            }
        }
    }
    map
}

static SUBTRACTIVE_FOG_RE: OnceLock<regex::Regex> = OnceLock::new();
fn get_subtractive_fog_regex() -> &'static regex::Regex {
    SUBTRACTIVE_FOG_RE.get_or_init(|| {
        regex::Regex::new(r"==マップフォグ.*?\[\s*[^,]+\s*,\s*[^,]+\s*,\s*(\d+)\s*,\s*[^,]+\s*,\s*2\s*\]==")
            .expect("valid subtractive fog regex")
    })
}

#[allow(clippy::too_many_arguments)]
fn audit_map_files(
    gen: RgssGeneration,
    source: &GameSourceIndex,
    resolver: &AssetResolver,
    graph: &mut DependencyGraph,
    node_ids: &mut HashSet<String>,
    findings: &mut Vec<AuditFinding>,
    finding_counter: &mut usize,
) {
    let ext = gen.data_extension();
    let tilesets_map = load_tilesets_map(source, ext);
    let mut map_paths: Vec<String> = source
        .files
        .values()
        .filter_map(|f| {
            if is_map_file(&f.logical_path, ext) {
                Some(f.logical_path.clone())
            } else {
                None
            }
        })
        .collect();
    map_paths.sort();

    for map_path in map_paths {
        let map_obj = match read_and_parse_data_file(&map_path, source, findings, finding_counter) {
            Some(m) => m,
            None => continue,
        };

        // Canonical map node ID: map:Data/Map001.rxdata
        let map_node_id = format!("map:{map_path}");
        let filename = map_path.rsplit('/').next().unwrap_or(&map_path);
        let map_name = filename
            .strip_suffix(&format!(".{ext}"))
            .unwrap_or(filename);
        let map_id = map_name
            .strip_prefix("Map")
            .or_else(|| map_name.strip_prefix("map"))
            .unwrap_or(map_name);

        if node_ids.insert(map_node_id.clone()) {
            graph.nodes.push(AssetNode {
                id: map_node_id.clone(),
                kind: AssetKind::Map,
                logical_path: Some(map_path.clone()),
            });
        }

        // Layer 0 & 1 tile inspection against tilesets (VX / VX Ace)
        if gen == RgssGeneration::Vx || gen == RgssGeneration::VxAce {
            let tileset_id = map_obj.get_ivar_i32("@tileset_id").unwrap_or(0);
            if let Some(MarshalValue::UserDefined { class_name, data }) = map_obj.get_ivar("@data") {
                if class_name == "Table" && data.len() >= 20 {
                    let xsize = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;
                    let ysize = u32::from_le_bytes([data[8], data[9], data[10], data[11]]) as usize;
                    let zsize = u32::from_le_bytes([data[12], data[13], data[14], data[15]]) as usize;
                    let total_size = u32::from_le_bytes([data[16], data[17], data[18], data[19]]) as usize;
                    let layer_size = xsize * ysize;
                    let tiles_bytes = &data[20..];

                    if tiles_bytes.len() >= total_size * 2 && layer_size > 0 {
                        let layers_to_scan = [0, 1];
                        for layer in layers_to_scan {
                            if layer < zsize {
                                let mut slot_counts: BTreeMap<usize, usize> = BTreeMap::new();
                                let start_idx = layer * layer_size;
                                let end_idx = (layer + 1) * layer_size;
                                for i in start_idx..end_idx {
                                    let tile_id = u16::from_le_bytes([
                                        tiles_bytes[i * 2],
                                        tiles_bytes[i * 2 + 1],
                                    ]);
                                    if let Some(slot) = tile_id_to_tileset_slot(tile_id) {
                                        *slot_counts.entry(slot).or_default() += 1;
                                    }
                                }

                                if let Some(names) = tilesets_map.get(&tileset_id) {
                                    for (slot, count) in slot_counts {
                                        if let Some(tileset_name) = names.get(slot) {
                                            if !tileset_name.is_empty() {
                                                let rule = CandidateRule::with_extensions(
                                                    &format!("Graphics/Tilesets/{tileset_name}"),
                                                    IMAGE_EXTENSIONS.iter().copied(),
                                                );
                                                let is_missing = match resolver.resolve(&rule, AssetKind::Image) {
                                                    ResolutionResult::Exact(_) | ResolutionResult::CaseMismatch { .. } => false,
                                                    _ => true,
                                                };
                                                if is_missing {
                                                    findings.push(make_finding(
                                                        DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING,
                                                        ReferenceRequiredness::Required,
                                                        format!(
                                                            "Map {map_id} Layer {layer} has {count} tiles referencing missing tileset '{tileset_name}'; floor/walls will render transparent/black"
                                                        ),
                                                        vec![map_node_id.clone()],
                                                        finding_counter,
                                                    ));
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Map note tag inspection for subtractive fog planes
        if let Some(note) = map_obj.get_ivar_str("@note") {
            let re = get_subtractive_fog_regex();
            for cap in re.captures_iter(note) {
                if let Some(z_match) = cap.get(1) {
                    if let Ok(z) = z_match.as_str().parse::<i32>() {
                        if z < 32 {
                            findings.push(make_finding(
                                DiagnosticCode::CMP_AUD_DYNAMIC_REFERENCE_UNRESOLVED,
                                ReferenceRequiredness::Optional,
                                format!(
                                    "Map {map_id} has subtractive fog Plane at Z={z} (below character layer Z=32); may cause black occlusion under OpenGL ES"
                                ),
                                vec![map_node_id.clone()],
                                finding_counter,
                            ));
                        }
                    }
                }
            }
        }

        // BGM / BGS
        if let Some(bgm) = map_obj.get_ivar("@bgm") {
            audit_audio_val(
                bgm,
                "Audio/BGM",
                &map_node_id,
                resolver,
                graph,
                node_ids,
                findings,
                finding_counter,
            );
        }
        if let Some(bgs) = map_obj.get_ivar("@bgs") {
            audit_audio_val(
                bgs,
                "Audio/BGS",
                &map_node_id,
                resolver,
                graph,
                node_ids,
                findings,
                finding_counter,
            );
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
                                node_ids,
                                findings,
                                &map_node_id,
                                &format!("{folder}/{name}"),
                                IMAGE_EXTENSIONS,
                                AssetKind::Image,
                                ReferenceRequiredness::Required,
                                finding_counter,
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
                            node_ids,
                            findings,
                            &map_node_id,
                            &format!("Graphics/Parallaxes/{name}"),
                            IMAGE_EXTENSIONS,
                            AssetKind::Image,
                            ReferenceRequiredness::Required,
                            finding_counter,
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
                                node_ids,
                                findings,
                                &map_node_id,
                                &format!("{folder}/{name}"),
                                IMAGE_EXTENSIONS,
                                AssetKind::Image,
                                ReferenceRequiredness::Required,
                                finding_counter,
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
                let event_node_id = format!("{map_name}:Event{event_id}");
                if node_ids.insert(event_node_id.clone()) {
                    graph.nodes.push(AssetNode {
                        id: event_node_id.clone(),
                        kind: AssetKind::Event,
                        logical_path: None,
                    });
                }
                graph.edges.push(ReferenceEdge {
                    source: map_node_id.clone(),
                    target: event_node_id.clone(),
                    relation: "contains_event".into(),
                    requiredness: ReferenceRequiredness::Required,
                    evidence: EvidenceTrail::through(EvidenceStage::ReferenceFound),
                    resolution: ReferenceResolution::Resolved,
                });

                if let Some(pages) = event.get_ivar("@pages").and_then(|v| v.as_array()) {
                    for (page_idx, page) in pages.iter().enumerate() {
                        let page_node_id = format!("{event_node_id}:Page{}", page_idx + 1);
                        if node_ids.insert(page_node_id.clone()) {
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
                                        node_ids,
                                        findings,
                                        &page_node_id,
                                        &format!("Graphics/Characters/{char_name}"),
                                        IMAGE_EXTENSIONS,
                                        AssetKind::Image,
                                        ReferenceRequiredness::Required,
                                        finding_counter,
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
                                                    node_ids,
                                                    findings,
                                                    &page_node_id,
                                                    &format!("Graphics/Characters/{g_name}"),
                                                    IMAGE_EXTENSIONS,
                                                    AssetKind::Image,
                                                    ReferenceRequiredness::Required,
                                                    finding_counter,
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
                                                node_ids,
                                                findings,
                                                finding_counter,
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
                                node_ids,
                                findings,
                                finding_counter,
                            );
                        }
                    }
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn audit_event_commands(
    commands: &[MarshalValue],
    provenance_prefix: &str,
    gen: RgssGeneration,
    resolver: &AssetResolver,
    graph: &mut DependencyGraph,
    node_ids: &mut HashSet<String>,
    findings: &mut Vec<AuditFinding>,
    finding_counter: &mut usize,
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

        if node_ids.insert(cmd_node_id.clone()) {
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
                            if node_ids.insert(node.id.clone()) {
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
                        ResolutionResult::CaseMismatch {
                            node,
                            expected,
                            actual,
                        } => {
                            findings.push(make_finding(
                                DiagnosticCode::CMP_AUD_CASE_MISMATCH,
                                ReferenceRequiredness::Required,
                                format!("Player transfer target map '{expected}' has case mismatch with '{actual}' on disk"),
                                vec![cmd_node_id.clone()],
                                finding_counter,
                            ));
                            if node_ids.insert(node.id.clone()) {
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
                            if node_ids.insert(target_id.clone()) {
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
                                finding_counter,
                            ));
                        }
                    }
                } else {
                    // Variable transfer: appointment_method == 1
                    let dyn_id = "unresolved_dynamic_reference:variable_transfer".to_string();
                    if node_ids.insert(dyn_id.clone()) {
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
                    if node_ids.insert(anim_target.clone()) {
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
                            node_ids,
                            findings,
                            &cmd_node_id,
                            &format!("Graphics/Pictures/{pic_name}"),
                            IMAGE_EXTENSIONS,
                            AssetKind::Image,
                            ReferenceRequiredness::Required,
                            finding_counter,
                        );
                    }
                }
            }
            // 241: Play BGM
            241 => {
                if let Some(audio) = params.first() {
                    audit_audio_val(
                        audio,
                        "Audio/BGM",
                        &cmd_node_id,
                        resolver,
                        graph,
                        node_ids,
                        findings,
                        finding_counter,
                    );
                }
            }
            // 245: Play BGS
            245 => {
                if let Some(audio) = params.first() {
                    audit_audio_val(
                        audio,
                        "Audio/BGS",
                        &cmd_node_id,
                        resolver,
                        graph,
                        node_ids,
                        findings,
                        finding_counter,
                    );
                }
            }
            // 249: Play ME
            249 => {
                if let Some(audio) = params.first() {
                    audit_audio_val(
                        audio,
                        "Audio/ME",
                        &cmd_node_id,
                        resolver,
                        graph,
                        node_ids,
                        findings,
                        finding_counter,
                    );
                }
            }
            // 250: Play SE
            250 => {
                if let Some(audio) = params.first() {
                    audit_audio_val(
                        audio,
                        "Audio/SE",
                        &cmd_node_id,
                        resolver,
                        graph,
                        node_ids,
                        findings,
                        finding_counter,
                    );
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
                                node_ids,
                                findings,
                                &cmd_node_id,
                                &format!("Graphics/Characters/{char_name}"),
                                IMAGE_EXTENSIONS,
                                AssetKind::Image,
                                ReferenceRequiredness::Required,
                                finding_counter,
                            );
                        }
                    }
                    if let Some(battler_name) = params.get(3).and_then(|v| v.as_str()) {
                        if !battler_name.is_empty() {
                            resolve_and_record(
                                resolver,
                                graph,
                                node_ids,
                                findings,
                                &cmd_node_id,
                                &format!("Graphics/Battlers/{battler_name}"),
                                IMAGE_EXTENSIONS,
                                AssetKind::Image,
                                ReferenceRequiredness::Required,
                                finding_counter,
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
                                node_ids,
                                findings,
                                &cmd_node_id,
                                &format!("Graphics/Characters/{char_name}"),
                                IMAGE_EXTENSIONS,
                                AssetKind::Image,
                                ReferenceRequiredness::Required,
                                finding_counter,
                            );
                        }
                    }
                    if let Some(face_name) = params.get(3).and_then(|v| v.as_str()) {
                        if !face_name.is_empty() {
                            resolve_and_record(
                                resolver,
                                graph,
                                node_ids,
                                findings,
                                &cmd_node_id,
                                &format!("Graphics/Faces/{face_name}"),
                                IMAGE_EXTENSIONS,
                                AssetKind::Image,
                                ReferenceRequiredness::Required,
                                finding_counter,
                            );
                        }
                    }
                }
            },
            _ => {}
        }
    }
}

fn has_cjk_or_japanese_punctuation(s: &str) -> bool {
    s.chars().any(|c| {
        let u = c as u32;
        // CJK Symbols and Punctuation (0x3000..=0x303F) including 「 (0x300C), 」 (0x300D), 、(0x3001), 。(0x3002)
        (0x3000..=0x303F).contains(&u)
            // Hiragana (0x3040..=0x309F)
            || (0x3040..=0x309F).contains(&u)
            // Katakana (0x30A0..=0x30FF)
            || (0x30A0..=0x30FF).contains(&u)
            // CJK Unified Ideographs (Kanji) (0x4E00..=0x9FFF)
            || (0x4E00..=0x9FFF).contains(&u)
            // Fullwidth forms (0xFF00..=0xFFEF)
            || (0xFF00..=0xFFEF).contains(&u)
    })
}

fn raw_bytes_contain_cjk_markers(bytes: &[u8]) -> bool {
    bytes.windows(3).any(|w| w == [0xE3, 0x80, 0x8C] || w == [0xE3, 0x80, 0x8D])
        || bytes.windows(2).any(|w| w == [0x81, 0x75] || w == [0x81, 0x76])
}

fn text_requests_font(s: &str) -> bool {
    s.contains("Font.default_name")
        || s.contains("VL Gothic")
        || s.contains("VL PGothic")
        || s.contains("UmePlus Gothic")
        || s.contains("MS Gothic")
        || s.contains("MS Mincho")
}

fn check_value_for_font_or_cjk(val: &MarshalValue) -> bool {
    if let Some(s) = val.as_str() {
        if has_cjk_or_japanese_punctuation(s) || text_requests_font(s) {
            return true;
        }
    }
    match val {
        MarshalValue::String(b) => raw_bytes_contain_cjk_markers(b),
        MarshalValue::InstanceVars { target, .. } => {
            if let MarshalValue::String(b) = &**target {
                raw_bytes_contain_cjk_markers(b)
            } else {
                false
            }
        }
        _ => false,
    }
}

fn try_read_and_parse_data_file(
    logical_path: &str,
    source: &GameSourceIndex,
) -> Option<MarshalValue> {
    let indexed_file = source.get_file(logical_path)?;
    let full_path = indexed_file.real_path(&source.root_path);
    let bytes = std::fs::read(&full_path).ok()?;
    parse_marshal(&bytes).ok()
}

fn inspect_scripts_for_font_cjk(ext: &str, source: &GameSourceIndex) -> Option<String> {
    let scripts_path = format!("Data/Scripts.{ext}");
    let indexed_file = source.get_file(&scripts_path)?;
    let full_path = indexed_file.real_path(&source.root_path);
    let bytes = std::fs::read(&full_path).ok()?;

    if let Ok(val) = parse_marshal(&bytes) {
        if let Some(arr) = val.as_array() {
            for entry in arr {
                if let Some(fields) = entry.as_array() {
                    let script_name = fields.get(1).and_then(|v| v.as_str()).unwrap_or("Script");
                    if let Some(script_data_val) = fields.get(2) {
                        let script_bytes = match script_data_val {
                            MarshalValue::String(b) => b.as_slice(),
                            MarshalValue::InstanceVars { target, .. } => {
                                if let MarshalValue::String(b) = &**target {
                                    b.as_slice()
                                } else {
                                    &[]
                                }
                            }
                            _ => &[],
                        };
                        let mut decompressed = Vec::new();
                        let text_bytes = {
                            use std::io::Read;
                            let mut decoder = flate2::read::ZlibDecoder::new(script_bytes);
                            if decoder.read_to_end(&mut decompressed).is_ok()
                                && !decompressed.is_empty()
                            {
                                decompressed.as_slice()
                            } else {
                                script_bytes
                            }
                        };
                        if let Ok(text) = std::str::from_utf8(text_bytes) {
                            if text_requests_font(text) {
                                return Some(format!(
                                    "script '{script_name}' requests specific font"
                                ));
                            }
                            if has_cjk_or_japanese_punctuation(text) {
                                return Some(format!(
                                    "script '{script_name}' contains CJK characters or Japanese punctuation"
                                ));
                            }
                        } else {
                            if raw_bytes_contain_cjk_markers(text_bytes) {
                                return Some(format!(
                                    "script '{script_name}' contains CJK characters or Japanese punctuation"
                                ));
                            }
                            if text_bytes.windows(9).any(|w| w == b"VL Gothic")
                                || text_bytes.windows(14).any(|w| w == b"UmePlus Gothic")
                                || text_bytes.windows(17).any(|w| w == b"Font.default_name")
                            {
                                return Some(format!(
                                    "script '{script_name}' requests specific font"
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
    // Also check raw bytes of Scripts file directly
    if raw_bytes_contain_cjk_markers(&bytes) {
        return Some("Scripts archive contains CJK quotation marks or punctuation".to_string());
    }
    if bytes.windows(9).any(|w| w == b"VL Gothic")
        || bytes.windows(14).any(|w| w == b"UmePlus Gothic")
        || bytes.windows(17).any(|w| w == b"Font.default_name")
    {
        return Some("Scripts archive requests font".to_string());
    }
    None
}

fn inspect_event_commands_for_font_cjk(commands: &[MarshalValue]) -> Option<String> {
    for cmd in commands {
        let code = cmd.get_ivar_i32("@code").unwrap_or(0);
        let params = cmd
            .get_ivar("@parameters")
            .and_then(|v| v.as_array())
            .unwrap_or(&[]);
        match code {
            401 | 108 | 408 | 355 | 655 => {
                if let Some(p0) = params.first() {
                    if check_value_for_font_or_cjk(p0) {
                        return Some(
                            "dialogue or event text contains Japanese quotation marks / CJK glyphs"
                                .to_string(),
                        );
                    }
                }
            }
            102 => {
                if let Some(choices) = params.first().and_then(|v| v.as_array()) {
                    for choice in choices {
                        if check_value_for_font_or_cjk(choice) {
                            return Some(
                                "choice text contains Japanese quotation marks / CJK glyphs"
                                    .to_string(),
                            );
                        }
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn inspect_maps_for_font_cjk(ext: &str, source: &GameSourceIndex) -> Option<String> {
    for file in source.files.values() {
        if is_map_file(&file.logical_path, ext) {
            if let Some(map_obj) = try_read_and_parse_data_file(&file.logical_path, source) {
                if let Some(events_hash) = map_obj.get_ivar("@events").and_then(|v| match v {
                    MarshalValue::Hash(entries) => Some(entries),
                    _ => None,
                }) {
                    for (_k, ev) in events_hash {
                        if let Some(pages) = ev.get_ivar("@pages").and_then(|v| v.as_array()) {
                            for page in pages {
                                if let Some(list) =
                                    page.get_ivar("@list").and_then(|v| v.as_array())
                                {
                                    if let Some(reason) = inspect_event_commands_for_font_cjk(list)
                                    {
                                        return Some(format!(
                                            "map '{}' {}",
                                            file.logical_path, reason
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

fn inspect_common_events_and_troops_for_font_cjk(
    ext: &str,
    source: &GameSourceIndex,
) -> Option<String> {
    let ce_path = format!("Data/CommonEvents.{ext}");
    if let Some(ce_val) = try_read_and_parse_data_file(&ce_path, source) {
        if let Some(arr) = ce_val.as_array() {
            for ce in arr {
                if let Some(list) = ce.get_ivar("@list").and_then(|v| v.as_array()) {
                    if let Some(reason) = inspect_event_commands_for_font_cjk(list) {
                        return Some(format!("CommonEvents {reason}"));
                    }
                }
            }
        }
    }
    let troops_path = format!("Data/Troops.{ext}");
    if let Some(troops_val) = try_read_and_parse_data_file(&troops_path, source) {
        if let Some(arr) = troops_val.as_array() {
            for troop in arr {
                if let Some(pages) = troop.get_ivar("@pages").and_then(|v| v.as_array()) {
                    for page in pages {
                        if let Some(list) = page.get_ivar("@list").and_then(|v| v.as_array()) {
                            if let Some(reason) = inspect_event_commands_for_font_cjk(list) {
                                return Some(format!("Troops {reason}"));
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

fn inspect_system_for_font_cjk(ext: &str, source: &GameSourceIndex) -> Option<String> {
    let sys_path = format!("Data/System.{ext}");
    if let Some(sys_val) = try_read_and_parse_data_file(&sys_path, source) {
        if let Some(title) = sys_val.get_ivar("@game_title").and_then(|v| v.as_str()) {
            if has_cjk_or_japanese_punctuation(title) || text_requests_font(title) {
                return Some(
                    "System game_title contains Japanese quotation marks / CJK glyphs".to_string(),
                );
            }
        }
    }
    None
}

fn inspect_database_entities_for_font_cjk(ext: &str, source: &GameSourceIndex) -> Option<String> {
    for db_name in &["Actors", "Items", "Skills", "Weapons", "Armors", "Enemies", "States"] {
        let db_path = format!("Data/{db_name}.{ext}");
        if let Some(val) = try_read_and_parse_data_file(&db_path, source) {
            if let Some(arr) = val.as_array() {
                for item in arr {
                    if let Some(name) = item.get_ivar("@name") {
                        if check_value_for_font_or_cjk(name) {
                            return Some(format!(
                                "{db_name} entity name contains Japanese punctuation or CJK glyphs"
                            ));
                        }
                    }
                    if let Some(desc) = item.get_ivar("@description") {
                        if check_value_for_font_or_cjk(desc) {
                            return Some(format!(
                                "{db_name} description contains Japanese punctuation or CJK glyphs"
                            ));
                        }
                    }
                }
            }
        }
    }
    None
}

fn audit_font_requirements(
    gen: RgssGeneration,
    source: &GameSourceIndex,
    coverage: &mut AuditCoverage,
    graph: &mut DependencyGraph,
    node_ids: &mut HashSet<String>,
    findings: &mut Vec<AuditFinding>,
    finding_counter: &mut usize,
) {
    let ext = gen.data_extension();

    let has_fonts = source.files.values().any(|f| {
        let lower = f.logical_path.to_ascii_lowercase();
        (lower.starts_with("fonts/") || lower.starts_with("fonts\\"))
            && (lower.ends_with(".ttf")
                || lower.ends_with(".otf")
                || lower.ends_with(".ttc")
                || lower.ends_with(".woff"))
    });

    if has_fonts {
        coverage.add_covered("fonts");
        for f in source.files.values() {
            let lower = f.logical_path.to_ascii_lowercase();
            if (lower.starts_with("fonts/") || lower.starts_with("fonts\\"))
                && (lower.ends_with(".ttf")
                    || lower.ends_with(".otf")
                    || lower.ends_with(".ttc")
                    || lower.ends_with(".woff"))
            {
                let font_id = format!("font:{}", f.logical_path);
                if node_ids.insert(font_id.clone()) {
                    graph.nodes.push(AssetNode {
                        id: font_id,
                        kind: AssetKind::Font,
                        logical_path: Some(f.logical_path.clone()),
                    });
                }
            }
        }
    }

    let requirement = inspect_scripts_for_font_cjk(ext, source)
        .or_else(|| inspect_maps_for_font_cjk(ext, source))
        .or_else(|| inspect_common_events_and_troops_for_font_cjk(ext, source))
        .or_else(|| inspect_system_for_font_cjk(ext, source))
        .or_else(|| inspect_database_entities_for_font_cjk(ext, source));

    let reason = match requirement {
        Some(r) => r,
        None => return,
    };

    if !has_fonts {
        coverage.add_unsupported("fonts", ReferenceRequiredness::Required);
        let font_target = "font:Fonts/VL-Gothic-Regular.ttf".to_string();
        if node_ids.insert(font_target.clone()) {
            graph.nodes.push(AssetNode {
                id: font_target.clone(),
                kind: AssetKind::Font,
                logical_path: Some("Fonts/VL-Gothic-Regular.ttf".to_string()),
            });
        }
        findings.push(make_finding(
            DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING,
            ReferenceRequiredness::Required,
            format!(
                "Font 'VL Gothic' or CJK glyph coverage is missing from 'Fonts/' directory ({reason})"
            ),
            vec![font_target],
            finding_counter,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_marshal_fixnum_decoding() {
        // 0
        assert_eq!(
            parse_marshal(&[0x04, 0x08, b'i', 0]).unwrap(),
            MarshalValue::Fixnum(0)
        );
        // Small positive 1 (1 + 5 = 6)
        assert_eq!(
            parse_marshal(&[0x04, 0x08, b'i', 6]).unwrap(),
            MarshalValue::Fixnum(1)
        );
        // Small positive 122 (122 + 5 = 127)
        assert_eq!(
            parse_marshal(&[0x04, 0x08, b'i', 127]).unwrap(),
            MarshalValue::Fixnum(122)
        );
        // 1-byte positive: 123 (count = 1, byte = 123)
        assert_eq!(
            parse_marshal(&[0x04, 0x08, b'i', 1, 123]).unwrap(),
            MarshalValue::Fixnum(123)
        );
        // 2-byte positive: 300
        assert_eq!(
            parse_marshal(&[0x04, 0x08, b'i', 2, 0x2c, 0x01]).unwrap(),
            MarshalValue::Fixnum(300)
        );
        // 3-byte positive: 100_000
        assert_eq!(
            parse_marshal(&[0x04, 0x08, b'i', 3, 0xa0, 0x86, 0x01]).unwrap(),
            MarshalValue::Fixnum(100_000)
        );
        // 4-byte positive: 1_000_000_000
        assert_eq!(
            parse_marshal(&[0x04, 0x08, b'i', 4, 0x00, 0xca, 0x9a, 0x3b]).unwrap(),
            MarshalValue::Fixnum(1_000_000_000)
        );

        // Small negative -1 (-1 - 5 = -6 -> 250 / 0xfa)
        assert_eq!(
            parse_marshal(&[0x04, 0x08, b'i', 0xfa]).unwrap(),
            MarshalValue::Fixnum(-1)
        );
        // Small negative -123 (-123 - 5 = -128 -> 128 / 0x80)
        assert_eq!(
            parse_marshal(&[0x04, 0x08, b'i', 0x80]).unwrap(),
            MarshalValue::Fixnum(-123)
        );
        // 1-byte negative: -124 (-1 as count -> -124)
        assert_eq!(
            parse_marshal(&[0x04, 0x08, b'i', 0xff, 0x84]).unwrap(),
            MarshalValue::Fixnum(-124)
        );
        // 4-byte negative: -100_000
        let neg_100k = (-100_000i32).to_le_bytes();
        assert_eq!(
            parse_marshal(&[
                0x04,
                0x08,
                b'i',
                0xfc,
                neg_100k[0],
                neg_100k[1],
                neg_100k[2],
                neg_100k[3]
            ])
            .unwrap(),
            MarshalValue::Fixnum(-100_000)
        );
    }

    #[test]
    fn test_bounded_allocation_rejects_huge_capacity() {
        // Array with prefix claiming 100_000 elements in a buffer of only 5 bytes
        let malformed = [0x04, 0x08, b'[', 3, 0xa0, 0x86, 0x01];
        let res = parse_marshal(&malformed);
        assert!(matches!(res, Err(MarshalError::Corrupt(_))));
    }

    #[test]
    fn test_instance_vars_wrapper_updates_object_slot() {
        // Ruby string "test" wrapped in I with ivar :@custom "val"
        // Followed by an array [@1] which links back to the wrapped object (slot 1)
        let data = [
            0x04, 0x08, b'[', 7, // array length 2 (2 + 5 = 7)
            b'I', b'"', 9, b'h', b'e', b'l', b'l', // string "hell" (len 4 -> 4 + 5 = 9)
            6,    // 1 ivar (1 + 5 = 6)
            b':', 7, b'@', b'f', // symbol :@f (len 2 -> 2 + 5 = 7)
            b'T', // true
            b'@', 6, // object link back to slot 1 (1 + 5 = 6)
        ];
        let val = parse_marshal(&data).unwrap();
        if let MarshalValue::Array(items) = val {
            assert_eq!(items.len(), 2);
            assert_eq!(items[0], items[1]);
        } else {
            panic!("Expected array");
        }
    }

    #[test]
    fn test_is_map_file_case_handling() {
        assert!(is_map_file("Data/Map001.rxdata", "rxdata"));
        assert!(is_map_file("Data/MAP001.rxdata", "rxdata"));
        assert!(is_map_file("data/map123.rvdata2", "rvdata2"));
        assert!(!is_map_file("Data/MapInfos.rxdata", "rxdata"));
        assert!(!is_map_file("Data/Map.rxdata", "rxdata"));
        assert!(!is_map_file("Data/Other.rxdata", "rxdata"));
    }
}
