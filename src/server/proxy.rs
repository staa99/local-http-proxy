use http_body_util::{combinators::BoxBody, BodyExt};
use hyper::{
    body::{Bytes, Incoming}, header::HOST, http, Request,
    Response,
    StatusCode,
    Uri,
};
use hyper_util::{
    client::legacy::{connect::HttpConnector, Client},
    rt::TokioExecutor,
};
use regex::Regex;
use std::{collections::HashMap, convert::Infallible, sync::LazyLock};

use crate::config::{AppConfig, ProxyMode};

#[derive(Debug, PartialEq, Eq)]
struct HostAndPath {
    host: String,
    path: String,
}

static LABEL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$").unwrap());
static HOST_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?P<key>[a-z0-9](?:[a-z0-9-]*[a-z0-9])?)\.[^:]+(?::\d+)?$").unwrap()
});
static PATH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^/(?P<key>[A-Za-z0-9](?:[A-Za-z0-9-]*[A-Za-z0-9])?)(?P<rest>(?:/[^?]*)?(?:\?.*)?)?$",
    )
    .unwrap()
});

static CLIENT: LazyLock<Client<HttpConnector, Incoming>> = LazyLock::new(|| {
    let mut http = HttpConnector::new();
    http.set_nodelay(true);
    Client::builder(TokioExecutor::new()).build(http)
});

pub async fn proxy_service(
    req: Request<Incoming>,
) -> Result<Response<BoxBody<Bytes, hyper::Error>>, Infallible> {
    let config = AppConfig::instance();
    let Some(destination) = get_destination(&req, &config.mode, &config.routes) else {
        return Ok(not_found());
    };

    let uri = match build_upstream_uri(&destination.host, &destination.path) {
        Some(u) => u,
        None => return Ok(bad_gateway()),
    };

    let (parts, body) = req.into_parts();
    let upstream_req = match build_upstream_request(parts, uri, body) {
        Some(r) => r,
        None => return Ok(internal_error()),
    };

    let res = match CLIENT.request(upstream_req).await {
        Ok(r) => r,
        Err(_) => return Ok(bad_gateway()),
    };

    let (parts, body) = res.into_parts();
    Ok(Response::from_parts(parts, body.boxed()))
}

/// Determines the destination URL based on the request and proxy mode.
///
/// Valid routing key rules:
/// - Only `[a-z0-9-]`
/// - Must not start or end with `-`
/// - No `.` or `/`
///
/// In Path mode: first path segment is the key.
/// In Domain mode: host must be `routing-key.localdomain`.
fn get_destination<B>(
    req: &Request<B>,
    mode: &ProxyMode,
    mapping: &HashMap<String, String>,
) -> Option<HostAndPath> {
    let (route_key, path) = match mode {
        ProxyMode::Domain => {
            let key = extract_key_from_host(req)?;
            let path = req
                .uri()
                .path_and_query()
                .map(|pq| pq.as_str().to_string())
                .unwrap_or_else(|| "/".to_string());
            (key, path)
        }
        ProxyMode::Path => {
            let caps = PATH_RE.captures(req.uri().path_and_query()?.as_str())?;
            let key = caps.name("key")?.as_str().to_ascii_lowercase();
            if !LABEL_RE.is_match(&key) {
                return None;
            }
            let mut path = caps.name("rest").map_or("/", |m| m.as_str()).to_string();
            if path.is_empty() {
                path = "/".to_string();
            }
            (key, path)
        }
    };

    Some(HostAndPath {
        host: mapping.get(&route_key)?.to_string(),
        path,
    })
}

fn extract_key_from_host<B>(req: &Request<B>) -> Option<String> {
    let host = req
        .headers()
        .get(HOST)?
        .to_str()
        .ok()?
        .trim()
        .to_ascii_lowercase();

    if !host.chars().next()?.is_ascii_alphanumeric() {
        return None;
    }

    let caps = HOST_RE.captures(&host)?;
    Some(caps.name("key")?.as_str().to_string())
}

fn boxed_full<T: Into<Bytes>>(data: T) -> BoxBody<Bytes, hyper::Error> {
    http_body_util::Full::new(data.into())
        .map_err(|never| match never {})
        .boxed()
}

fn not_found() -> Response<BoxBody<Bytes, hyper::Error>> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(boxed_full("Local Http Proxy: Route Not Found"))
        .unwrap()
}

fn bad_gateway() -> Response<BoxBody<Bytes, hyper::Error>> {
    Response::builder()
        .status(StatusCode::BAD_GATEWAY)
        .body(boxed_full("Local Http Proxy: Bad Gateway"))
        .unwrap()
}

fn internal_error() -> Response<BoxBody<Bytes, hyper::Error>> {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .body(boxed_full("Local Http Proxy: Internal Error"))
        .unwrap()
}

fn build_upstream_uri(host: &str, path: &str) -> Option<Uri> {
    let uri = format!("{}{}", host, path);
    uri.parse().ok()
}

fn build_upstream_request(
    parts: http::request::Parts,
    uri: Uri,
    body: Incoming,
) -> Option<Request<Incoming>> {
    let mut builder = Request::builder()
        .method(parts.method)
        .version(parts.version)
        .uri(uri);

    if let Some(headers) = builder.headers_mut() {
        for (k, v) in parts.headers.iter() {
            if k != HOST {
                headers.insert(k, v.clone());
            }
        }
    } else {
        return None;
    }

    builder.body(body).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyper::Request;

    fn mapping(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    // --- Host (Domain) mode ---

    #[test]
    fn host_mode_key_dot_localdomain_ok() {
        let req = Request::builder()
            .uri("/v1/users?limit=10")
            .header(HOST, "api.localhost:8080")
            .body(())
            .unwrap();

        let map = mapping(&[("api", "http://upstream-api")]);
        let got = get_destination(&req, &ProxyMode::Domain, &map).unwrap();

        assert_eq!(
            got,
            HostAndPath {
                host: "http://upstream-api".into(),
                path: "/v1/users?limit=10".into()
            }
        );
    }

    #[test]
    fn host_mode_hyphen_edges_invalid() {
        for h in ["-api.local", "api-.local"] {
            let req = Request::builder()
                .uri("/")
                .header(HOST, h)
                .body(())
                .unwrap();
            let map = mapping(&[("api", "http://x")]);
            assert!(get_destination(&req, &ProxyMode::Domain, &map).is_none());
        }
    }

    #[test]
    fn host_mode_requires_dot_between_key_and_localdomain() {
        let req = Request::builder()
            .uri("/health")
            .header(HOST, "api")
            .body(())
            .unwrap();

        let map = mapping(&[("api", "http://upstream")]);
        assert!(get_destination(&req, &ProxyMode::Domain, &map).is_none());
    }

    #[test]
    fn host_mode_ipv6_literal_rejected() {
        let req = Request::builder()
            .uri("/ping?x=1")
            .header(HOST, "[::1]:3000")
            .body(())
            .unwrap();

        let map = mapping(&[("::1", "http://local-ipv6")]);
        assert!(get_destination(&req, &ProxyMode::Domain, &map).is_none());
    }

    // --- Path mode ---

    #[test]
    fn path_mode_basic_with_query() {
        let req = Request::builder().uri("/svc/status?x=1").body(()).unwrap();

        let map = mapping(&[("svc", "http://upstream-svc")]);
        let got = get_destination(&req, &ProxyMode::Path, &map).unwrap();
        assert_eq!(
            got,
            HostAndPath {
                host: "http://upstream-svc".into(),
                path: "/status?x=1".into()
            }
        );
    }

    #[test]
    fn path_mode_only_prefix_becomes_root() {
        let req = Request::builder().uri("/svc").body(()).unwrap();

        let map = mapping(&[("svc", "http://upstream-svc")]);
        let got = get_destination(&req, &ProxyMode::Path, &map).unwrap();
        assert_eq!(
            got,
            HostAndPath {
                host: "http://upstream-svc".into(),
                path: "/".into()
            }
        );
    }

    #[test]
    fn path_mode_root_is_none() {
        let req = Request::builder().uri("/").body(()).unwrap();
        let map = mapping(&[("svc", "http://upstream-svc")]);
        assert!(get_destination(&req, &ProxyMode::Path, &map).is_none());
    }

    #[test]
    fn path_mode_invalid_key_rejected() {
        let req = Request::builder().uri("/-bad/users").body(()).unwrap();
        let map = mapping(&[("-bad", "http://x")]);
        assert!(get_destination(&req, &ProxyMode::Path, &map).is_none());
    }
}
