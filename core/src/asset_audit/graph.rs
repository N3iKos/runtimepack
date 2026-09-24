use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AssetKind {
    Map,
    Event,
    EventPage,
    Command,
    DatabaseEntity,
    Script,
    Plugin,
    Image,
    Audio,
    Font,
    Video,
    ArchiveEntry,
    ExternalResource,
    RuntimeProvidedResource,
    UnresolvedDynamicReference,
}

impl AssetKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Map => "map",
            Self::Event => "event",
            Self::EventPage => "event_page",
            Self::Command => "command",
            Self::DatabaseEntity => "database_entity",
            Self::Script => "script",
            Self::Plugin => "plugin",
            Self::Image => "image",
            Self::Audio => "audio",
            Self::Font => "font",
            Self::Video => "video",
            Self::ArchiveEntry => "archive_entry",
            Self::ExternalResource => "external_resource",
            Self::RuntimeProvidedResource => "runtime_provided_resource",
            Self::UnresolvedDynamicReference => "unresolved_dynamic_reference",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetNode {
    pub id: String,
    pub kind: AssetKind,
    pub logical_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReferenceResolution {
    Resolved,
    Unresolved,
    External,
    RuntimeProvided,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceEdge {
    pub source: String,
    pub target: String,
    pub relation: String,
    #[serde(default)]
    pub requiredness: super::ReferenceRequiredness,
    pub evidence: super::EvidenceTrail,
    pub resolution: ReferenceResolution,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyGraph {
    pub nodes: Vec<AssetNode>,
    pub edges: Vec<ReferenceEdge>,
}

impl DependencyGraph {
    pub(crate) fn canonicalize(&mut self) {
        self.nodes.sort_by(|left, right| {
            (&left.id, &left.kind, &left.logical_path).cmp(&(
                &right.id,
                &right.kind,
                &right.logical_path,
            ))
        });
        self.nodes.dedup();
        self.edges.sort_by(|left, right| {
            (
                &left.source,
                &left.target,
                &left.relation,
                &left.requiredness,
                &left.evidence,
                &left.resolution,
            )
                .cmp(&(
                    &right.source,
                    &right.target,
                    &right.relation,
                    &right.requiredness,
                    &right.evidence,
                    &right.resolution,
                ))
        });
        self.edges.dedup();
    }
}
