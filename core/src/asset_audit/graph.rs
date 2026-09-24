use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
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
    }
}
