//! Asking the web for a file, for what Muddie takes from it: a map, the
//! plugins. The file is asked for only if it changed since the version
//! known of it.

use std::time::Duration;

/// How the web is asked for files: for no longer than `patience`.
pub fn agent(patience: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(patience))
        .max_redirects(3)
        .user_agent("muddie")
        .build()
        .into()
}

/// What tells this version of a file on the web from the next: its tag,
/// or failing that when it last changed.
pub fn version_header<B>(response: &ureq::http::Response<B>) -> Option<String> {
    let header = |name: &str| {
        (response.headers().get(name))
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
    };
    header("etag").or_else(|| header("last-modified"))
}

/// Bytes fetched, with the version the web gave of them; none when the
/// web had nothing newer.
pub type Fetched = Option<(Vec<u8>, Option<String>)>;

/// The file at `source`, no larger than `limit`, if it is not the version
/// `known`. Errors are said without the address.
pub fn get(
    source: &str,
    known: Option<&str>,
    limit: u64,
    patience: Duration,
) -> Result<Fetched, String> {
    let mut request = agent(patience).get(source.trim());
    if let Some(known) = known {
        request = request.header("If-None-Match", known);
    }
    let mut response = match request.call() {
        Err(ureq::Error::StatusCode(304)) => return Ok(None),
        other => other.map_err(|error| error.to_string())?,
    };
    if response.status().as_u16() == 304 {
        return Ok(None);
    }
    let version = version_header(&response);
    let bytes = (response.body_mut().with_config().limit(limit).read_to_vec())
        .map_err(|error| error.to_string())?;
    Ok(Some((bytes, version)))
}
