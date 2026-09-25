use crate::asset_audit::{
    audit_finding_classification, AssetAuditOptions, AuditFinding, FindingDisposition,
    FindingSeverity, ReferenceRequiredness,
};
use crate::diagnostics::DiagnosticCode;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkAuditResult {
    Verified {
        url: String,
        status_code: u16,
        content_type: Option<String>,
    },
    UnverifiedOffline {
        url: String,
        reason: String,
    },
    NotFound {
        url: String,
        status_code: Option<u16>,
        error: String,
    },
    InvalidUrl {
        url: String,
        error: String,
    },
}

impl NetworkAuditResult {
    pub fn is_verified(&self) -> bool {
        matches!(self, Self::Verified { .. })
    }

    pub fn to_finding(&self, source_node_id: Option<&str>) -> Option<AuditFinding> {
        match self {
            Self::Verified { .. } => None,
            Self::UnverifiedOffline { url, reason } => {
                let (severity, disposition) = audit_finding_classification(
                    DiagnosticCode::CMP_AUD_EXTERNAL_RESOURCE_UNVERIFIED,
                    ReferenceRequiredness::Required,
                    false,
                )
                .unwrap_or((FindingSeverity::Warning, FindingDisposition::Indeterminate));

                let message = if reason.is_empty()
                    || reason == "Network audit is disabled (--allow-network-audit not specified)"
                {
                    format!(
                        "External resource URL '{url}' is unverified in offline audit mode (pass --allow-network-audit to verify)"
                    )
                } else {
                    format!("External resource URL '{url}' is unverified: {reason}")
                };

                Some(AuditFinding {
                    id: format!(
                        "{}-{}",
                        DiagnosticCode::CMP_AUD_EXTERNAL_RESOURCE_UNVERIFIED,
                        sanitize_id(url)
                    ),
                    code: DiagnosticCode::CMP_AUD_EXTERNAL_RESOURCE_UNVERIFIED.to_string(),
                    severity,
                    disposition,
                    requiredness: ReferenceRequiredness::Required,
                    message,
                    source_node_ids: source_node_id
                        .map(|s| vec![s.to_string()])
                        .unwrap_or_default(),
                    orphan: false,
                })
            }
            Self::NotFound {
                url,
                status_code,
                error,
            } => {
                let status_desc = status_code
                    .map(|s| format!(" (status {s})"))
                    .unwrap_or_default();
                Some(AuditFinding {
                    id: format!(
                        "{}-{}",
                        DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING,
                        sanitize_id(url)
                    ),
                    code: DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING.to_string(),
                    severity: FindingSeverity::Error,
                    disposition: FindingDisposition::Blocking,
                    requiredness: ReferenceRequiredness::Required,
                    message: format!(
                        "External network resource '{url}' was not found{status_desc}: {error}"
                    ),
                    source_node_ids: source_node_id
                        .map(|s| vec![s.to_string()])
                        .unwrap_or_default(),
                    orphan: false,
                })
            }
            Self::InvalidUrl { url, error } => Some(AuditFinding {
                id: format!(
                    "{}-{}",
                    DiagnosticCode::CMP_AUD_MALFORMED_RESOURCE,
                    sanitize_id(url)
                ),
                code: DiagnosticCode::CMP_AUD_MALFORMED_RESOURCE.to_string(),
                severity: FindingSeverity::Error,
                disposition: FindingDisposition::Blocking,
                requiredness: ReferenceRequiredness::Required,
                message: format!("External resource URL '{url}' is invalid: {error}"),
                source_node_ids: source_node_id
                    .map(|s| vec![s.to_string()])
                    .unwrap_or_default(),
                orphan: false,
            }),
        }
    }
}

fn sanitize_id(input: &str) -> String {
    input
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

struct ParsedUrl {
    scheme: String,
    host: String,
    port: u16,
    path_and_query: String,
}

fn parse_url(url_str: &str) -> Result<ParsedUrl, String> {
    let trimmed = url_str.trim();
    let lower = trimmed.to_ascii_lowercase();

    let (scheme, rest) = if lower.starts_with("http://") {
        ("http".to_string(), &trimmed[7..])
    } else if lower.starts_with("https://") {
        ("https".to_string(), &trimmed[8..])
    } else {
        return Err(format!(
            "Scheme not permitted; only http:// and https:// are supported: '{trimmed}'"
        ));
    };

    let (host_port, path_part) = match rest.find(|c| c == '/' || c == '?') {
        Some(pos) => {
            let hp = &rest[..pos];
            if rest.as_bytes()[pos] == b'?' {
                (hp, format!("/{}", &rest[pos..]))
            } else {
                (hp, rest[pos..].to_string())
            }
        }
        None => (rest, "/".to_string()),
    };

    let (host, port) = if let Some((h, p)) = host_port.split_once(':') {
        let port_num = p
            .parse::<u16>()
            .map_err(|_| format!("Invalid port in URL: '{p}'"))?;
        (h.to_string(), port_num)
    } else {
        let default_port = if scheme == "http" { 80 } else { 443 };
        (host_port.to_string(), default_port)
    };

    if host.is_empty() {
        return Err(format!("Empty host in URL: '{trimmed}'"));
    }

    Ok(ParsedUrl {
        scheme,
        host,
        port,
        path_and_query: path_part.to_string(),
    })
}

pub fn audit_network_reference(url_str: &str, options: &AssetAuditOptions) -> NetworkAuditResult {
    let parsed = match parse_url(url_str) {
        Ok(p) => p,
        Err(err) => {
            return NetworkAuditResult::InvalidUrl {
                url: url_str.to_string(),
                error: err,
            }
        }
    };

    if !options.allow_network {
        return NetworkAuditResult::UnverifiedOffline {
            url: url_str.to_string(),
            reason: "Network audit is disabled (--allow-network-audit not specified)".to_string(),
        };
    }

    let timeout = Duration::from_secs(5);
    let max_body_bytes = options.limits.max_single_file_bytes;

    execute_bounded_request(url_str, &parsed, 3, timeout, max_body_bytes)
}

fn execute_bounded_request(
    url_str: &str,
    parsed: &ParsedUrl,
    redirects_left: usize,
    timeout: Duration,
    max_body_bytes: u64,
) -> NetworkAuditResult {
    if redirects_left == 0 {
        return NetworkAuditResult::NotFound {
            url: url_str.to_string(),
            status_code: None,
            error: "Redirect limit exceeded (maximum 3 redirects allowed)".to_string(),
        };
    }

    if parsed.scheme == "https" {
        return NetworkAuditResult::UnverifiedOffline {
            url: url_str.to_string(),
            reason:
                "HTTPS verification requires TLS support (transport unavailable in static sandbox)"
                    .to_string(),
        };
    }

    use std::io::{Read, Write};
    use std::net::{TcpStream, ToSocketAddrs};

    let addr_str = format!("{}:{}", parsed.host, parsed.port);
    let socket_addrs = match addr_str.to_socket_addrs() {
        Ok(mut addrs) => match addrs.next() {
            Some(a) => a,
            None => {
                return NetworkAuditResult::NotFound {
                    url: url_str.to_string(),
                    status_code: None,
                    error: format!("No IP addresses found for host '{}'", parsed.host),
                }
            }
        },
        Err(err) => {
            return NetworkAuditResult::NotFound {
                url: url_str.to_string(),
                status_code: None,
                error: format!("DNS resolution failed for '{addr_str}': {err}"),
            }
        }
    };

    let mut stream = match TcpStream::connect_timeout(&socket_addrs, timeout) {
        Ok(s) => s,
        Err(err) => {
            return NetworkAuditResult::NotFound {
                url: url_str.to_string(),
                status_code: None,
                error: format!("Connection failed to '{addr_str}': {err}"),
            }
        }
    };

    let _ = stream.set_read_timeout(Some(timeout));
    let _ = stream.set_write_timeout(Some(timeout));

    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: RuntimePack-AssetAudit/0.1\r\nConnection: close\r\nAccept: */*\r\n\r\n",
        parsed.path_and_query, parsed.host
    );

    if let Err(err) = stream.write_all(request.as_bytes()) {
        return NetworkAuditResult::NotFound {
            url: url_str.to_string(),
            status_code: None,
            error: format!("Failed to send HTTP request: {err}"),
        };
    }

    let mut header_buf = Vec::new();
    let mut temp = [0u8; 1024];
    let max_header_bytes = 65536.min(max_body_bytes as usize);

    while header_buf.len() < max_header_bytes {
        match stream.read(&mut temp) {
            Ok(0) => break,
            Ok(n) => {
                header_buf.extend_from_slice(&temp[..n]);
                if header_buf.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            Err(err) => {
                return NetworkAuditResult::NotFound {
                    url: url_str.to_string(),
                    status_code: None,
                    error: format!("Failed to read HTTP response: {err}"),
                };
            }
        }
    }

    let header_str = String::from_utf8_lossy(&header_buf);
    let mut lines = header_str.lines();
    let status_line = match lines.next() {
        Some(line) => line,
        None => {
            return NetworkAuditResult::NotFound {
                url: url_str.to_string(),
                status_code: None,
                error: "Empty HTTP response from server".to_string(),
            }
        }
    };

    let status_parts: Vec<&str> = status_line.split_whitespace().collect();
    if status_parts.len() < 2 {
        return NetworkAuditResult::NotFound {
            url: url_str.to_string(),
            status_code: None,
            error: format!("Malformed HTTP status line: '{status_line}'"),
        };
    }

    let status_code: u16 = match status_parts[1].parse() {
        Ok(code) => code,
        Err(_) => {
            return NetworkAuditResult::NotFound {
                url: url_str.to_string(),
                status_code: None,
                error: format!("Invalid HTTP status code in: '{status_line}'"),
            }
        }
    };

    let mut content_type = None;
    let mut location = None;

    for line in lines {
        if line.is_empty() {
            break;
        }
        if let Some((name, val)) = line.split_once(':') {
            let name_trim = name.trim().to_ascii_lowercase();
            let val_trim = val.trim().to_string();
            if name_trim == "content-type" {
                content_type = Some(val_trim);
            } else if name_trim == "location" {
                location = Some(val_trim);
            }
        }
    }

    if matches!(status_code, 301..=303 | 307..=308) {
        if let Some(loc) = location {
            let target_redirect = if loc.starts_with("http://") || loc.starts_with("https://") {
                loc.clone()
            } else {
                let base_prefix = format!("http://{}:{}", parsed.host, parsed.port);
                if loc.starts_with('/') {
                    format!("{base_prefix}{loc}")
                } else {
                    format!("{base_prefix}/{loc}")
                }
            };
            if let Ok(next_parsed) = parse_url(&target_redirect) {
                return execute_bounded_request(
                    &target_redirect,
                    &next_parsed,
                    redirects_left - 1,
                    timeout,
                    max_body_bytes,
                );
            } else {
                return NetworkAuditResult::NotFound {
                    url: url_str.to_string(),
                    status_code: Some(status_code),
                    error: format!("Invalid redirect target URL: '{loc}'"),
                };
            }
        } else {
            return NetworkAuditResult::NotFound {
                url: url_str.to_string(),
                status_code: Some(status_code),
                error: "Redirect status without Location header".to_string(),
            };
        }
    }

    if (200..=299).contains(&status_code) {
        NetworkAuditResult::Verified {
            url: url_str.to_string(),
            status_code,
            content_type,
        }
    } else {
        NetworkAuditResult::NotFound {
            url: url_str.to_string(),
            status_code: Some(status_code),
            error: format!("HTTP response status {status_code}"),
        }
    }
}
