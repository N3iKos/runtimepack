use crate::asset_audit::{
    AssetAuditReport, AssetNode, AuditOutcome, CanonicalAuditArtifacts, FindingSeverity,
    ReferenceEdge, ReferenceResolution,
};
use crate::diagnostics::DiagnosticError;
use std::collections::HashMap;

pub fn escape_html(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(c),
        }
    }
    escaped
}

pub fn render_audit_html(
    artifacts: &CanonicalAuditArtifacts,
) -> Result<Vec<u8>, DiagnosticError> {
    let report: AssetAuditReport = serde_json::from_slice(&artifacts.report_json).map_err(|e| {
        DiagnosticError::from_code(
            "CMP-AUD-010",
            format!("Failed to parse canonical audit report: {e}"),
        )
    })?;

    // Node lookup map
    let node_map: HashMap<&str, &AssetNode> = report
        .graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();

    // Edge lookup by source
    let mut edges_by_source: HashMap<&str, Vec<&ReferenceEdge>> = HashMap::new();
    for edge in &report.graph.edges {
        edges_by_source
            .entry(edge.source.as_str())
            .or_default()
            .push(edge);
    }

    // Counters
    let total_assets = report.graph.nodes.len();
    let resolved_references = report
        .graph
        .edges
        .iter()
        .filter(|e| e.resolution == ReferenceResolution::Resolved)
        .count();
    let missing_references = report
        .findings
        .iter()
        .filter(|f| f.code == "CMP-AUD-002" || f.code == "CMP-AUD-003")
        .count();
    let case_mismatches = report
        .findings
        .iter()
        .filter(|f| f.code == "CMP-AUD-004" || f.code == "CMP-AUD-005")
        .count();
    let unresolved_dynamics = report
        .findings
        .iter()
        .filter(|f| f.code == "CMP-AUD-006")
        .count();
    let unreadable_resources = report
        .findings
        .iter()
        .filter(|f| f.code == "CMP-AUD-008")
        .count();

    let outcome_str = match report.outcome {
        AuditOutcome::Pass => "PASS",
        AuditOutcome::Warning => "WARNING",
        AuditOutcome::Fail => "FAIL",
        AuditOutcome::Indeterminate => "INDETERMINATE",
    };

    let outcome_class = match report.outcome {
        AuditOutcome::Pass => "badge-pass",
        AuditOutcome::Warning => "badge-warning",
        AuditOutcome::Fail => "badge-fail",
        AuditOutcome::Indeterminate => "badge-indeterminate",
    };

    let mode_str = format!("{:?}", report.mode).to_uppercase();
    let engine_str = report.engine.to_string();

    let json_text = String::from_utf8_lossy(&artifacts.report_json);
    let safe_embedded_json = json_text
        .replace("</script", "<\\/script")
        .replace("</Script", "<\\/Script");

    let mut html = String::with_capacity(64 * 1024);

    html.push_str("<!DOCTYPE html>\n");
    html.push_str("<html lang=\"en\">\n<head>\n");
    html.push_str("<meta charset=\"UTF-8\">\n");
    html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n");
    html.push_str("<title>RuntimePack Asset Audit Report</title>\n");
    html.push_str("<style>\n");
    html.push_str(
        r#"
:root {
  --bg-main: #0f172a;
  --bg-card: #1e293b;
  --bg-hover: #334155;
  --border: #334155;
  --text-main: #f8fafc;
  --text-muted: #94a3b8;
  --accent: #38bdf8;
  --accent-hover: #0ea5e9;
  --pass-bg: #064e3b;
  --pass-text: #34d399;
  --pass-border: #059669;
  --warning-bg: #78350f;
  --warning-text: #fbbf24;
  --warning-border: #d97706;
  --fail-bg: #7f1d1d;
  --fail-text: #f87171;
  --fail-border: #dc2626;
  --indet-bg: #3b0764;
  --indet-text: #c084fc;
  --indet-border: #7e22ce;
  --info-bg: #0c4a6e;
  --info-text: #38bdf8;
  --info-border: #0284c7;
}

* { box-sizing: border-box; margin: 0; padding: 0; }

body {
  background-color: var(--bg-main);
  color: var(--text-main);
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Oxygen, Ubuntu, Cantarell, "Helvetica Neue", sans-serif;
  line-height: 1.5;
  padding: 2rem;
}

.container {
  max-width: 1200px;
  margin: 0 auto;
}

header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding-bottom: 1.5rem;
  border-bottom: 1px solid var(--border);
  margin-bottom: 2rem;
  flex-wrap: wrap;
  gap: 1rem;
}

.header-title h1 {
  font-size: 1.75rem;
  font-weight: 700;
  color: var(--accent);
  letter-spacing: -0.025em;
}

.header-title p {
  font-size: 0.875rem;
  color: var(--text-muted);
}

.header-badges {
  display: flex;
  gap: 0.75rem;
  align-items: center;
}

.badge {
  padding: 0.35rem 0.85rem;
  border-radius: 9999px;
  font-size: 0.8125rem;
  font-weight: 600;
  letter-spacing: 0.05em;
  text-transform: uppercase;
  border: 1px solid transparent;
}

.badge-engine {
  background-color: var(--bg-card);
  color: var(--text-main);
  border-color: var(--border);
}

.badge-mode {
  background-color: var(--bg-card);
  color: var(--accent);
  border-color: var(--accent);
}

.badge-pass {
  background-color: var(--pass-bg);
  color: var(--pass-text);
  border-color: var(--pass-border);
}

.badge-warning {
  background-color: var(--warning-bg);
  color: var(--warning-text);
  border-color: var(--warning-border);
}

.badge-fail {
  background-color: var(--fail-bg);
  color: var(--fail-text);
  border-color: var(--fail-border);
}

.badge-indeterminate {
  background-color: var(--indet-bg);
  color: var(--indet-text);
  border-color: var(--indet-border);
}

.badge-info {
  background-color: var(--info-bg);
  color: var(--info-text);
  border-color: var(--info-border);
}

.summary-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
  gap: 1rem;
  margin-bottom: 2rem;
}

.stat-card {
  background-color: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 0.5rem;
  padding: 1.25rem;
  text-align: center;
}

.stat-card .stat-value {
  font-size: 2rem;
  font-weight: 700;
  color: var(--text-main);
  margin-bottom: 0.25rem;
}

.stat-card .stat-label {
  font-size: 0.8125rem;
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.stat-missing .stat-value { color: var(--fail-text); }
.stat-case .stat-value { color: var(--warning-text); }
.stat-resolved .stat-value { color: var(--pass-text); }
.stat-dynamic .stat-value { color: var(--indet-text); }
.stat-unreadable .stat-value { color: var(--fail-text); }

.controls-panel {
  background-color: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 0.5rem;
  padding: 1.25rem;
  margin-bottom: 1.5rem;
  display: flex;
  gap: 1rem;
  align-items: center;
  flex-wrap: wrap;
}

.search-box {
  flex: 1;
  min-width: 240px;
}

.search-box input {
  width: 100%;
  background-color: var(--bg-main);
  border: 1px solid var(--border);
  color: var(--text-main);
  padding: 0.6rem 1rem;
  border-radius: 0.375rem;
  font-size: 0.875rem;
}

.search-box input:focus {
  outline: none;
  border-color: var(--accent);
}

.filter-group {
  display: flex;
  gap: 0.5rem;
  align-items: center;
}

.filter-btn {
  background-color: var(--bg-main);
  border: 1px solid var(--border);
  color: var(--text-muted);
  padding: 0.5rem 0.875rem;
  border-radius: 0.375rem;
  font-size: 0.8125rem;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.15s ease;
}

.filter-btn:hover {
  background-color: var(--bg-hover);
  color: var(--text-main);
}

.filter-btn.active {
  background-color: var(--accent);
  color: var(--bg-main);
  border-color: var(--accent);
}

select.filter-select {
  background-color: var(--bg-main);
  border: 1px solid var(--border);
  color: var(--text-main);
  padding: 0.55rem 1rem;
  border-radius: 0.375rem;
  font-size: 0.875rem;
  cursor: pointer;
}

select.filter-select:focus {
  outline: none;
  border-color: var(--accent);
}

.table-container {
  background-color: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 0.5rem;
  overflow: hidden;
}

table {
  width: 100%;
  border-collapse: collapse;
  text-align: left;
  font-size: 0.875rem;
}

thead {
  background-color: #172033;
  border-bottom: 2px solid var(--border);
}

th {
  padding: 0.875rem 1rem;
  font-weight: 600;
  color: var(--text-muted);
  text-transform: uppercase;
  font-size: 0.75rem;
  letter-spacing: 0.05em;
}

tbody tr.finding-row {
  border-bottom: 1px solid var(--border);
  cursor: pointer;
  transition: background-color 0.15s ease;
}

tbody tr.finding-row:hover {
  background-color: #243248;
}

td {
  padding: 0.875rem 1rem;
}

td.toggle-col {
  width: 32px;
  text-align: center;
  color: var(--text-muted);
  font-size: 1.1rem;
  font-weight: bold;
}

.code-badge {
  font-family: ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace;
  font-size: 0.75rem;
  background-color: var(--bg-main);
  padding: 0.2rem 0.5rem;
  border-radius: 0.25rem;
  border: 1px solid var(--border);
  color: var(--accent);
  display: inline-block;
}

.details-row {
  display: none;
  background-color: #131c2e;
  border-bottom: 1px solid var(--border);
}

.details-row.expanded {
  display: table-row;
}

.details-content {
  padding: 1.25rem 1.5rem;
  font-size: 0.8125rem;
}

.details-section {
  margin-bottom: 1rem;
}

.details-section:last-child {
  margin-bottom: 0;
}

.details-section-title {
  font-weight: 600;
  color: var(--accent);
  margin-bottom: 0.5rem;
  text-transform: uppercase;
  font-size: 0.75rem;
  letter-spacing: 0.05em;
}

.details-list {
  list-style: none;
  padding-left: 0;
}

.details-list li {
  padding: 0.25rem 0;
  font-family: ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace;
  color: var(--text-muted);
}

.details-list li strong {
  color: var(--text-main);
}

.evidence-trail {
  display: flex;
  flex-wrap: wrap;
  gap: 0.35rem;
  margin-top: 0.25rem;
}

.stage-tag {
  background-color: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 0.25rem;
  padding: 0.15rem 0.5rem;
  font-size: 0.7rem;
  color: var(--text-muted);
}

.empty-state {
  padding: 3rem;
  text-align: center;
  color: var(--text-muted);
}

footer {
  margin-top: 2rem;
  text-align: center;
  font-size: 0.8125rem;
  color: var(--text-muted);
  border-top: 1px solid var(--border);
  padding-top: 1.5rem;
}
"#);
    html.push_str("</style>\n</head>\n<body>\n");

    html.push_str("<div class=\"container\">\n");

    // Header
    html.push_str("<header>\n");
    html.push_str("  <div class=\"header-title\">\n");
    html.push_str("    <h1>RuntimePack Asset Audit</h1>\n");
    html.push_str(&format!(
        "    <p>Static dependency &amp; reference validation report &bull; Schema v{}</p>\n",
        report.schema_version
    ));
    html.push_str("  </div>\n");
    html.push_str("  <div class=\"header-badges\">\n");
    html.push_str(&format!(
        "    <span class=\"badge badge-engine\">{}</span>\n",
        escape_html(&engine_str)
    ));
    html.push_str(&format!(
        "    <span class=\"badge badge-mode\">{}</span>\n",
        escape_html(&mode_str)
    ));
    html.push_str(&format!(
        "    <span class=\"badge {}\">{}</span>\n",
        outcome_class, outcome_str
    ));
    html.push_str("  </div>\n");
    html.push_str("</header>\n");

    // Summary Counters Grid
    html.push_str("<div class=\"summary-grid\">\n");

    // Total Assets
    html.push_str("  <div class=\"stat-card\">\n");
    html.push_str(&format!(
        "    <div class=\"stat-value\" id=\"stat-total-assets\">{}</div>\n",
        total_assets
    ));
    html.push_str("    <div class=\"stat-label\">Total Assets</div>\n");
    html.push_str("  </div>\n");

    // Resolved
    html.push_str("  <div class=\"stat-card stat-resolved\">\n");
    html.push_str(&format!(
        "    <div class=\"stat-value\" id=\"stat-resolved\">{}</div>\n",
        resolved_references
    ));
    html.push_str("    <div class=\"stat-label\">Resolved</div>\n");
    html.push_str("  </div>\n");

    // Missing
    html.push_str("  <div class=\"stat-card stat-missing\">\n");
    html.push_str(&format!(
        "    <div class=\"stat-value\" id=\"stat-missing\">{}</div>\n",
        missing_references
    ));
    html.push_str("    <div class=\"stat-label\">Missing</div>\n");
    html.push_str("  </div>\n");

    // Case Mismatch
    html.push_str("  <div class=\"stat-card stat-case\">\n");
    html.push_str(&format!(
        "    <div class=\"stat-value\" id=\"stat-case-mismatch\">{}</div>\n",
        case_mismatches
    ));
    html.push_str("    <div class=\"stat-label\">Case Mismatch</div>\n");
    html.push_str("  </div>\n");

    // Unresolved Dynamic
    html.push_str("  <div class=\"stat-card stat-dynamic\">\n");
    html.push_str(&format!(
        "    <div class=\"stat-value\" id=\"stat-unresolved-dynamic\">{}</div>\n",
        unresolved_dynamics
    ));
    html.push_str("    <div class=\"stat-label\">Unresolved Dynamic</div>\n");
    html.push_str("  </div>\n");

    // Unreadable
    html.push_str("  <div class=\"stat-card stat-unreadable\">\n");
    html.push_str(&format!(
        "    <div class=\"stat-value\" id=\"stat-unreadable\">{}</div>\n",
        unreadable_resources
    ));
    html.push_str("    <div class=\"stat-label\">Unreadable</div>\n");
    html.push_str("  </div>\n");

    html.push_str("</div>\n");

    // Controls Panel
    html.push_str("<div class=\"controls-panel\">\n");
    html.push_str("  <div class=\"search-box\">\n");
    html.push_str("    <input type=\"text\" id=\"search-input\" placeholder=\"Search findings (message, code, source path)...\" />\n");
    html.push_str("  </div>\n");

    html.push_str("  <div class=\"filter-group\">\n");
    html.push_str("    <button type=\"button\" class=\"filter-btn active\" data-severity=\"all\">All</button>\n");
    html.push_str("    <button type=\"button\" class=\"filter-btn\" data-severity=\"ERROR\">Error</button>\n");
    html.push_str("    <button type=\"button\" class=\"filter-btn\" data-severity=\"WARNING\">Warning</button>\n");
    html.push_str("    <button type=\"button\" class=\"filter-btn\" data-severity=\"INFO\">Info</button>\n");
    html.push_str("  </div>\n");

    html.push_str("  <div class=\"filter-group\">\n");
    html.push_str("    <select id=\"stage-filter\" class=\"filter-select\">\n");
    html.push_str("      <option value=\"all\">All Stages</option>\n");
    html.push_str("      <option value=\"REFERENCE_FOUND\">Reference Found</option>\n");
    html.push_str("      <option value=\"PATH_RESOLVED\">Path Resolved</option>\n");
    html.push_str("      <option value=\"FILE_EXISTS\">File Exists</option>\n");
    html.push_str("      <option value=\"FILE_READABLE\">File Readable</option>\n");
    html.push_str("      <option value=\"INTEGRITY_VERIFIED\">Integrity Verified</option>\n");
    html.push_str("      <option value=\"DECODE_SUCCESS\">Decode Success</option>\n");
    html.push_str("      <option value=\"ENGINE_LOAD_SUCCESS\">Engine Load Success</option>\n");
    html.push_str("      <option value=\"RUNTIME_USE_SUCCESS\">Runtime Use Success</option>\n");
    html.push_str("    </select>\n");
    html.push_str("  </div>\n");

    html.push_str("  <div class=\"filter-group\">\n");
    html.push_str("    <button type=\"button\" id=\"btn-toggle-all\" class=\"filter-btn\">Expand All</button>\n");
    html.push_str("  </div>\n");

    html.push_str("</div>\n");

    // Findings Table
    html.push_str("<div class=\"table-container\">\n");
    html.push_str("<table id=\"findings-table\">\n");
    html.push_str("  <thead>\n");
    html.push_str("    <tr>\n");
    html.push_str("      <th class=\"toggle-col\"></th>\n");
    html.push_str("      <th>Severity</th>\n");
    html.push_str("      <th>Code</th>\n");
    html.push_str("      <th>Message</th>\n");
    html.push_str("      <th>Requiredness</th>\n");
    html.push_str("      <th>Disposition</th>\n");
    html.push_str("      <th>Sources</th>\n");
    html.push_str("    </tr>\n");
    html.push_str("  </thead>\n");
    html.push_str("  <tbody id=\"findings-body\">\n");

    if report.findings.is_empty() {
        html.push_str("    <tr id=\"no-findings-row\"><td colspan=\"7\" class=\"empty-state\">No findings recorded. All asset references are valid.</td></tr>\n");
    } else {
        for (idx, finding) in report.findings.iter().enumerate() {
            let (sev_str, sev_badge_class) = match finding.severity {
                FindingSeverity::Error => ("ERROR", "badge-fail"),
                FindingSeverity::Warning => ("WARNING", "badge-warning"),
                FindingSeverity::Info => ("INFO", "badge-info"),
            };

            let req_str = format!("{:?}", finding.requiredness).to_uppercase();
            let disp_str = format!("{:?}", finding.disposition).to_uppercase();
            let sources_count = finding.source_node_ids.len();

            // Collect evidence stages for this finding from edges connected to sources
            let mut stages_for_finding = Vec::new();
            for source_id in &finding.source_node_ids {
                if let Some(edges) = edges_by_source.get(source_id.as_str()) {
                    for edge in edges {
                        for stage in edge.evidence.stages() {
                            let stage_name = format!("{stage:?}").to_uppercase();
                            if !stages_for_finding.contains(&stage_name) {
                                stages_for_finding.push(stage_name);
                            }
                        }
                    }
                }
            }
            let stages_attr = stages_for_finding.join(" ");

            html.push_str(&format!(
                "    <tr class=\"finding-row\" data-index=\"{idx}\" data-severity=\"{sev_str}\" data-stages=\"{stages_attr}\">\n"
            ));
            html.push_str("      <td class=\"toggle-col\">+</td>\n");
            html.push_str(&format!(
                "      <td><span class=\"badge {}\">{}</span></td>\n",
                sev_badge_class, sev_str
            ));
            html.push_str(&format!(
                "      <td><span class=\"code-badge\">{}</span></td>\n",
                escape_html(&finding.code)
            ));
            html.push_str(&format!(
                "      <td>{}</td>\n",
                escape_html(&finding.message)
            ));
            html.push_str(&format!(
                "      <td><span class=\"badge badge-engine\">{}</span></td>\n",
                escape_html(&req_str)
            ));
            html.push_str(&format!(
                "      <td><span class=\"badge badge-engine\">{}</span></td>\n",
                escape_html(&disp_str)
            ));
            html.push_str(&format!("      <td>{}</td>\n", sources_count));
            html.push_str("    </tr>\n");

            // Expandable details row
            html.push_str(&format!(
                "    <tr class=\"details-row\" id=\"details-{idx}\">\n"
            ));
            html.push_str("      <td colspan=\"7\"><div class=\"details-content\">\n");

            // Finding metadata
            html.push_str("        <div class=\"details-section\">\n");
            html.push_str("          <div class=\"details-section-title\">Finding Details</div>\n");
            html.push_str("          <ul class=\"details-list\">\n");
            html.push_str(&format!(
                "            <li><strong>ID:</strong> {}</li>\n",
                escape_html(&finding.id)
            ));
            html.push_str(&format!(
                "            <li><strong>Orphan Reference:</strong> {}</li>\n",
                finding.orphan
            ));
            html.push_str("          </ul>\n");
            html.push_str("        </div>\n");

            // Provenance source nodes
            html.push_str("        <div class=\"details-section\">\n");
            html.push_str("          <div class=\"details-section-title\">Referencing Sources &amp; Provenance</div>\n");
            if finding.source_node_ids.is_empty() {
                html.push_str("          <p class=\"details-list\">No specific source nodes recorded.</p>\n");
            } else {
                html.push_str("          <ul class=\"details-list\">\n");
                for src_id in &finding.source_node_ids {
                    if let Some(node) = node_map.get(src_id.as_str()) {
                        let path_display = node
                            .logical_path
                            .as_deref()
                            .unwrap_or("<none>");
                        html.push_str(&format!(
                            "            <li><strong>[{:?}]</strong> {} &rarr; <code>{}</code></li>\n",
                            node.kind,
                            escape_html(&node.id),
                            escape_html(path_display)
                        ));
                    } else {
                        html.push_str(&format!(
                            "            <li><code>{}</code></li>\n",
                            escape_html(src_id)
                        ));
                    }

                    // Display edges associated with this source
                    if let Some(edges) = edges_by_source.get(src_id.as_str()) {
                        for edge in edges {
                            let res_str = format!("{:?}", edge.resolution).to_uppercase();
                            let req_str = format!("{:?}", edge.requiredness).to_uppercase();
                            html.push_str(&format!(
                                "              <div style=\"padding-left: 1.5rem; margin: 0.25rem 0;\">\n\
                                               &bull; <strong>{}</strong> &rarr; <code>{}</code> [{}] ({})\n\
                                               <div class=\"evidence-trail\">\n",
                                escape_html(&edge.relation),
                                escape_html(&edge.target),
                                escape_html(&res_str),
                                escape_html(&req_str),
                            ));
                            for stage in edge.evidence.stages() {
                                html.push_str(&format!(
                                    "                <span class=\"stage-tag\">{:?}</span>\n",
                                    stage
                                ));
                            }
                            html.push_str("              </div></div>\n");
                        }
                    }
                }
                html.push_str("          </ul>\n");
            }
            html.push_str("        </div>\n");

            html.push_str("      </div></td>\n");
            html.push_str("    </tr>\n");
        }
    }

    html.push_str("  </tbody>\n");
    html.push_str("</table>\n");
    html.push_str("</div>\n");

    // Footer
    html.push_str("<footer>\n");
    html.push_str("  <p>RuntimePack Standalone Build Compatibility System &bull; Offline Audit Report Viewer</p>\n");
    html.push_str("</footer>\n");

    html.push_str("</div>\n");

    // Embedded canonical JSON payload
    html.push_str("<script type=\"application/json\" id=\"audit-data\">\n");
    html.push_str(&safe_embedded_json);
    if !safe_embedded_json.ends_with('\n') {
        html.push('\n');
    }
    html.push_str("</script>\n");

    // Client-side vanilla JS for search, filters, expand/collapse
    html.push_str("<script>\n");
    html.push_str(
        r#"
(function() {
  var searchInput = document.getElementById('search-input');
  var severityButtons = document.querySelectorAll('.filter-btn[data-severity]');
  var stageSelect = document.getElementById('stage-filter');
  var toggleAllBtn = document.getElementById('btn-toggle-all');
  var findingRows = document.querySelectorAll('tr.finding-row');
  var allExpanded = false;
  var currentSeverity = 'all';

  // Row expand / collapse on click
  findingRows.forEach(function(row) {
    row.addEventListener('click', function(e) {
      var idx = row.getAttribute('data-index');
      var details = document.getElementById('details-' + idx);
      var toggle = row.querySelector('.toggle-col');
      if (details) {
        var isExpanded = details.classList.contains('expanded');
        if (isExpanded) {
          details.classList.remove('expanded');
          if (toggle) toggle.textContent = '+';
        } else {
          details.classList.add('expanded');
          if (toggle) toggle.textContent = '−';
        }
      }
    });
  });

  // Toggle all
  if (toggleAllBtn) {
    toggleAllBtn.addEventListener('click', function() {
      allExpanded = !allExpanded;
      toggleAllBtn.textContent = allExpanded ? 'Collapse All' : 'Expand All';
      document.querySelectorAll('.details-row').forEach(function(details) {
        if (allExpanded) {
          details.classList.add('expanded');
        } else {
          details.classList.remove('expanded');
        }
      });
      document.querySelectorAll('.toggle-col').forEach(function(toggle) {
        toggle.textContent = allExpanded ? '−' : '+';
      });
    });
  }

  function applyFilters() {
    var query = (searchInput ? searchInput.value : '').toLowerCase().trim();
    var stage = stageSelect ? stageSelect.value : 'all';

    findingRows.forEach(function(row) {
      var idx = row.getAttribute('data-index');
      var details = document.getElementById('details-' + idx);
      var severity = row.getAttribute('data-severity');
      var stages = (row.getAttribute('data-stages') || '').split(' ');
      var rowText = (row.textContent + ' ' + (details ? details.textContent : '')).toLowerCase();

      var matchesSearch = !query || rowText.indexOf(query) !== -1;
      var matchesSeverity = (currentSeverity === 'all') || (severity === currentSeverity);
      var matchesStage = (stage === 'all') || (stages.indexOf(stage) !== -1);

      if (matchesSearch && matchesSeverity && matchesStage) {
        row.style.display = '';
        if (details && details.classList.contains('expanded')) {
          details.style.display = '';
        }
      } else {
        row.style.display = 'none';
        if (details) {
          details.style.display = 'none';
        }
      }
    });
  }

  if (searchInput) {
    searchInput.addEventListener('input', applyFilters);
  }

  severityButtons.forEach(function(btn) {
    btn.addEventListener('click', function() {
      severityButtons.forEach(function(b) { b.classList.remove('active'); });
      btn.classList.add('active');
      currentSeverity = btn.getAttribute('data-severity');
      applyFilters();
    });
  });

  if (stageSelect) {
    stageSelect.addEventListener('change', applyFilters);
  }
})();
"#,
    );
    html.push_str("</script>\n");

    html.push_str("</body>\n</html>\n");

    Ok(html.into_bytes())
}
