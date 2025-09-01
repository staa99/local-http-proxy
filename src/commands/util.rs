use hyper::Uri;
use regex::Regex;
use std::sync::LazyLock;

static LABEL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$").unwrap());

const INVALID_SOURCE_RULES: &str = "The name must be 1–63 characters long and a single segment usable in both a URL path and a domain.

  - Max length: 63 characters.
  - Allowed: lowercase letters (a-z), digits (0-9), and hyphens (-).
  - Must not start or end with a hyphen.
  - Must not contain '.' or '/'.

  Examples: 'my-app', 'api', 'project1'";

/// Normalize a user-provided source identifier into the canonical routing key used by the proxy.
/// Rules:
/// - Accepts forms like "/app", "app", "app.localhost", "app.localhost:3000".
/// - Extracts the first path segment or the first host label before a dot.
/// - Lowercases and validates against the proxy's label rules: [a-z0-9-], not starting/ending with '-'.
pub fn normalize_source_key(input: &str) -> Result<String, String> {
    let s = input.trim();
    if s.is_empty() {
        return Err("Source cannot be empty".to_string());
    }

    let key = parse_source_raw_key(s)?.to_ascii_lowercase();
    validate_source_label(&key)?;
    Ok(key)
}

/// Normalize a user-provided target into an absolute HTTP URI string acceptable by the proxy.
/// Rules:
/// - Allow just a port (e.g., "3000" or ":3000") -> http://localhost:3000
/// - Allow host:port or IP:port -> http://{host}:port
/// - Allow IPv6 literals in brackets: "\[::1]:3000" -> http://\[::1]:3000
/// - Allow explicit http://...; reject https:// (not supported by current client)
/// - Trim trailing slashes to avoid '//' when concatenating with request path
pub fn normalize_target(input: &str) -> Result<String, String> {
    fn is_all_digits(s: &str) -> bool {
        !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
    }

    fn parse_and_validate_http(s: &str) -> Result<Uri, String> {
        let uri: Uri = s
            .parse()
            .map_err(|_| "Target must be a valid absolute URI or host:port".to_string())?;
        if uri.scheme_str() != Some("http") {
            return Err("Only http:// targets are supported".into());
        }
        if uri.authority().is_none() {
            return Err("Target must include a host (authority)".into());
        }
        Ok(uri)
    }

    let s = input.trim();
    if s.is_empty() {
        return Err("Target cannot be empty".into());
    }

    // Port-only forms
    let with_scheme = if is_all_digits(s) {
        format!("http://localhost:{}", s)
    } else if let Some(rest) = s.strip_prefix(':') {
        if is_all_digits(rest) {
            format!("http://localhost:{}", rest)
        } else {
            return Err("Invalid port after ':' in target".into());
        }
    } else if s.starts_with("http://") {
        s.to_string()
    } else if s.starts_with("https://") {
        return Err("https:// upstreams are not supported (TLS not enabled). Use http:// or a port like 3000".into());
    } else if s.contains("://") {
        return Err("Unsupported URI scheme. Only http:// is supported".into());
    } else {
        format!("http://{}", s)
    };

    // Validate and normalize trailing slash
    let uri = parse_and_validate_http(&with_scheme)?;
    let out = uri.to_string().trim_end_matches('/').to_string();
    Ok(out)
}

fn parse_source_raw_key(s: &str) -> Result<String, String> {
    if s.starts_with('/') {
        return s
            .trim_start_matches('/')
            .split('/')
            .find(|seg| !seg.is_empty())
            .map(|seg| seg.to_string())
            .ok_or_else(|| {
                "Invalid source: path must include a non-empty first segment (e.g., /api)"
                    .to_string()
            });
    }

    if s.contains('.') {
        // strip :port if present, then take first label before '.'
        let before_port = s.split(':').next().unwrap_or(s);
        return before_port
            .split('.')
            .find(|seg| !seg.is_empty())
            .map(|seg| seg.to_string())
            .ok_or_else(|| {
                "Invalid source: hostname must start with a non-empty label (e.g., api.localhost)"
                    .to_string()
            });
    }

    Ok(s.to_string())
}

fn validate_source_label(key: &str) -> Result<(), String> {
    if key.len() > 63 {
        return Err(format!(
            "Invalid source name: \"{}\".\n\n  {}",
            key, INVALID_SOURCE_RULES
        ));
    }
    if !LABEL_RE.is_match(key) {
        return Err(format!(
            "Invalid source name: \"{}\".\n\n  {}",
            key, INVALID_SOURCE_RULES
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_source_accepts_path_and_host_forms() {
        assert_eq!(normalize_source_key("/Svc").unwrap(), "svc");
        assert_eq!(normalize_source_key("API.localhost").unwrap(), "api");
        assert_eq!(normalize_source_key("api.localhost:8080").unwrap(), "api");
        assert_eq!(normalize_source_key("my-app").unwrap(), "my-app");
        // dotted host inputs normalize to the first label
        assert_eq!(normalize_source_key("has.dot").unwrap(), "has");
    }

    #[test]
    fn normalize_source_rejects_invalid() {
        assert!(normalize_source_key("").is_err());
        assert!(normalize_source_key("-bad").is_err());
        assert!(normalize_source_key("bad-").is_err());
        assert!(normalize_source_key("has/slash").is_err());
        // 64-char label rejected
        let sixty_four = "a".repeat(64);
        assert!(normalize_source_key(&sixty_four).is_err());
        // 63-char label accepted
        let sixty_three = "a".repeat(63);
        assert_eq!(normalize_source_key(&sixty_three).unwrap(), sixty_three);
    }

    #[test]
    fn normalize_target_supports_port_and_hostport() {
        assert_eq!(normalize_target("3000").unwrap(), "http://localhost:3000");
        assert_eq!(normalize_target(":3000").unwrap(), "http://localhost:3000");
        assert_eq!(
            normalize_target("localhost:3000").unwrap(),
            "http://localhost:3000"
        );
        assert_eq!(
            normalize_target("127.0.0.1:8080").unwrap(),
            "http://127.0.0.1:8080"
        );
        assert_eq!(normalize_target("[::1]:8080").unwrap(), "http://[::1]:8080");
        assert_eq!(
            normalize_target("http://svc:8080/").unwrap(),
            "http://svc:8080"
        );
    }

    #[test]
    fn normalize_target_rejects_https_and_bad_scheme() {
        assert!(normalize_target("https://host").is_err());
        assert!(normalize_target("ftp://host").is_err());
    }
}
