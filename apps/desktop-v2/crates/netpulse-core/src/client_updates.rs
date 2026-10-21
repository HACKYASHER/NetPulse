//! User-initiated installer updates from the project's official public releases.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};
pub const REPOSITORY: &str = "HACKYASHER/NetPulse";
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Release {
    pub tag: String,
    pub page: String,
    pub url: String,
    pub sha256: String,
    pub size: u64,
    pub prerelease: bool,
}
fn invalid() -> Error {
    Error::new(
        "ClientUpdateInvalid",
        "client-update",
        "Invalid NetPulse release metadata or checksum",
    )
}
fn version(v: &str) -> Option<(u64, u64, u64, u8, u64)> {
    let (base, pre) = v
        .trim_start_matches('v')
        .split_once('-')
        .map_or((v.trim_start_matches('v'), None), |(a, b)| (a, Some(b)));
    let mut n = base.split('.').map(str::parse::<u64>);
    let (a, b, c) = (n.next()?.ok()?, n.next()?.ok()?, n.next()?.ok()?);
    if n.next().is_some() {
        return None;
    }
    let (rank, num) = if let Some(p) = pre {
        let (label, num) = p.split_once('.')?;
        (
            match label {
                "alpha" => 0,
                "beta" => 1,
                "rc" => 2,
                _ => return None,
            },
            num.parse().ok()?,
        )
    } else {
        (3, 0)
    };
    Some((a, b, c, rank, num))
}
fn validate(r: &Release) -> Result<()> {
    if version(&r.tag).is_none()
        || !r
            .tag
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-".contains(&b))
        || r.sha256.len() != 64
        || !r.sha256.bytes().all(|b| b.is_ascii_hexdigit())
        || !(1024..=256 * 1024 * 1024).contains(&r.size)
        || r.page != format!("https://github.com/{REPOSITORY}/releases/tag/{}", r.tag)
        || r.url
            != format!(
                "https://github.com/{REPOSITORY}/releases/download/{}/NetPulse-Setup.exe",
                r.tag
            )
    {
        return Err(invalid());
    }
    Ok(())
}
fn metadata(endpoint: &str) -> Result<serde_json::Value> {
    let response = crate::network::client(Duration::from_secs(20))?
        .get(endpoint)
        .header("User-Agent", "NetPulse-client-updater")
        .header("Accept", "application/vnd.github+json")
        .send()
        .map_err(|_| {
            Error::new(
                "ClientUpdateUnavailable",
                "client-update",
                "Could not reach GitHub",
            )
        })?;
    if !response.status().is_success() {
        return Err(Error::new(
            "ClientUpdateUnavailable",
            "client-update",
            format!("GitHub HTTP {}", response.status().as_u16()),
        ));
    }
    let mut bytes = Vec::new();
    response
        .take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| Error::io("client-update-read", e))?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err(invalid());
    }
    serde_json::from_slice(&bytes).map_err(|_| invalid())
}
fn parse(v: &serde_json::Value) -> Result<Release> {
    if v["draft"] == true {
        return Err(invalid());
    }
    let a = v["assets"]
        .as_array()
        .ok_or_else(invalid)?
        .iter()
        .find(|a| a["name"] == "NetPulse-Setup.exe" && a["state"] == "uploaded")
        .ok_or_else(invalid)?;
    let r = Release {
        tag: v["tag_name"].as_str().ok_or_else(invalid)?.into(),
        page: v["html_url"].as_str().ok_or_else(invalid)?.into(),
        url: a["browser_download_url"]
            .as_str()
            .ok_or_else(invalid)?
            .into(),
        sha256: a["digest"]
            .as_str()
            .and_then(|s| s.strip_prefix("sha256:"))
            .ok_or_else(invalid)?
            .into(),
        size: a["size"].as_u64().ok_or_else(invalid)?,
        prerelease: v["prerelease"].as_bool().unwrap_or(false),
    };
    validate(&r)?;
    Ok(r)
}
pub fn check(current: &str) -> Result<Option<Release>> {
    let values = metadata(&format!(
        "https://api.github.com/repos/{REPOSITORY}/releases?per_page=30"
    ))?;
    let mut releases: Vec<_> = values
        .as_array()
        .ok_or_else(invalid)?
        .iter()
        .filter_map(|v| parse(v).ok())
        .filter(|r| version(&r.tag) > version(current))
        .collect();
    releases.sort_by_key(|r| std::cmp::Reverse(version(&r.tag)));
    Ok(releases.into_iter().next())
}
pub fn revalidate(r: &Release) -> Result<()> {
    validate(r)?;
    if parse(&metadata(&format!(
        "https://api.github.com/repos/{REPOSITORY}/releases/tags/{}",
        r.tag
    ))?)?
        != *r
    {
        return Err(invalid());
    }
    Ok(())
}
fn destination(dir: &Path, r: &Release) -> PathBuf {
    dir.join("updates").join(&r.tag).join("NetPulse-Setup.exe")
}
pub fn verify_download(dir: &Path, r: &Release) -> Result<PathBuf> {
    validate(r)?;
    let path = destination(dir, r);
    let mut f = std::fs::File::open(&path).map_err(|e| Error::io("client-update-file", e))?;
    if f.metadata()
        .map_err(|e| Error::io("client-update-file", e))?
        .len()
        != r.size
    {
        return Err(invalid());
    }
    let mut h = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = f
            .read(&mut buf)
            .map_err(|e| Error::io("client-update-hash", e))?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    let hash = h
        .finalize()
        .iter()
        .map(|x| format!("{x:02x}"))
        .collect::<String>();
    if hash != r.sha256.to_lowercase() {
        return Err(invalid());
    }
    Ok(path)
}
pub fn download(dir: &Path, r: &Release) -> Result<()> {
    revalidate(r)?;
    if verify_download(dir, r).is_ok() {
        return Ok(());
    }
    let path = destination(dir, r);
    std::fs::create_dir_all(path.parent().unwrap())
        .map_err(|e| Error::io("client-update-folder", e))?;
    let partial = path.with_extension(format!("{}.partial", uuid::Uuid::new_v4()));
    let result = (|| {
        let response = crate::network::client(Duration::from_secs(300))?
            .get(&r.url)
            .header("User-Agent", "NetPulse-client-updater")
            .send()
            .map_err(|_| {
                Error::new(
                    "ClientDownloadFailed",
                    "client-update",
                    "Installer download failed",
                )
            })?;
        if !response.status().is_success() || response.url().scheme() != "https" {
            return Err(invalid());
        }
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&partial)
            .map_err(|e| Error::io("client-update-write", e))?;
        let copied = std::io::copy(&mut response.take(r.size + 1), &mut f)
            .map_err(|e| Error::io("client-update-download", e))?;
        f.flush()
            .and_then(|_| f.sync_all())
            .map_err(|e| Error::io("client-update-write", e))?;
        drop(f);
        if copied != r.size {
            return Err(invalid());
        }
        let bytes = std::fs::read(&partial).map_err(|e| Error::io("client-update-hash", e))?;
        let digest = Sha256::digest(&bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        if bytes.get(..2) != Some(b"MZ") || digest != r.sha256.to_lowercase() {
            return Err(invalid());
        }
        std::fs::rename(&partial, &path).map_err(|e| Error::io("client-update-activate", e))?;
        Ok(())
    })();
    if partial.exists() {
        let _ = std::fs::remove_file(partial);
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versions_and_source_are_constrained() {
        assert!(version("2.0.0-alpha.10") > version("2.0.0-alpha.6"));
        assert!(version("2.0.0") > version("2.0.0-rc.9"));
        assert!(version("../../x").is_none());
        let mut r = Release {
            tag: "v2.0.0-alpha.7".into(),
            page: format!("https://github.com/{REPOSITORY}/releases/tag/v2.0.0-alpha.7"),
            url: format!(
                "https://github.com/{REPOSITORY}/releases/download/v2.0.0-alpha.7/NetPulse-Setup.exe"
            ),
            sha256: "a".repeat(64),
            size: 2048,
            prerelease: true,
        };
        assert!(validate(&r).is_ok());
        r.url = "https://example.com/installer.exe".into();
        assert!(validate(&r).is_err());
    }
    #[test]
    fn altered_cached_installer_is_rejected() {
        let d = tempfile::tempdir().unwrap();
        let bytes = vec![42u8; 2048];
        let r = Release {
            tag: "v2.0.0-alpha.7".into(),
            page: format!("https://github.com/{REPOSITORY}/releases/tag/v2.0.0-alpha.7"),
            url: format!(
                "https://github.com/{REPOSITORY}/releases/download/v2.0.0-alpha.7/NetPulse-Setup.exe"
            ),
            sha256: Sha256::digest(&bytes)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect(),
            size: 2048,
            prerelease: true,
        };
        let p = destination(d.path(), &r);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, &bytes).unwrap();
        assert!(verify_download(d.path(), &r).is_ok());
        std::fs::write(p, vec![43u8; 2048]).unwrap();
        assert!(verify_download(d.path(), &r).is_err());
    }
}
