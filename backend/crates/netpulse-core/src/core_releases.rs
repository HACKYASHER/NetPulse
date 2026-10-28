//! Releases are selected from official GitHub repositories, including prereleases.
//! Elevated installation independently verifies the original archive before trust.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{io::Read, path::Path, time::Duration};
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Release {
    pub engine: String,
    pub tag: String,
    pub prerelease: bool,
    pub page: String,
    pub asset: String,
    pub url: String,
    pub archive_sha256: String,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Approval {
    pub release: Release,
    pub exe_sha256: String,
    pub dll_sha256: Option<String>,
}
fn repo(engine: &str) -> Result<&'static str> {
    match engine {
        "xray" => Ok("XTLS/Xray-core"),
        "sing-box" => Ok("SagerNet/sing-box"),
        _ => Err(Error::new("UnknownEngine", "core-update", "Unknown engine")),
    }
}
pub fn manifest_name(engine: &str) -> String {
    format!("{engine}-release.json")
}
fn invalid() -> Error {
    Error::new(
        "CoreReleaseInvalid",
        "core-update",
        "Invalid official release metadata",
    )
}
fn hex(bytes: impl AsRef<[u8]>) -> String {
    bytes.as_ref().iter().map(|b| format!("{b:02x}")).collect()
}
fn valid_sha(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|c| c.is_ascii_hexdigit())
}
fn validate(release: &Release) -> Result<()> {
    let repo = repo(&release.engine)?;
    if release.tag.is_empty()
        || release.tag.len() > 80
        || !release
            .tag
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-_".contains(&b))
        || !valid_sha(&release.archive_sha256)
    {
        return Err(invalid());
    }
    if release.page != format!("https://github.com/{repo}/releases/tag/{}", release.tag)
        || release.url
            != format!(
                "https://github.com/{repo}/releases/download/{}/{}",
                release.tag, release.asset
            )
    {
        return Err(invalid());
    }
    if (release.engine == "xray" && release.asset != "Xray-windows-64.zip")
        || (release.engine == "sing-box"
            && release.asset
                != format!(
                    "sing-box-{}-windows-amd64.zip",
                    release.tag.trim_start_matches('v')
                ))
    {
        return Err(invalid());
    }
    Ok(())
}
fn metadata(url: &str) -> Result<serde_json::Value> {
    let response = crate::network::update_client(Duration::from_secs(12))?
        .get(url)
        .header("User-Agent", "NetPulse/2.0")
        .header("Accept", "application/vnd.github+json")
        .send()
        .map_err(|error| {
            Error::new(
                "CoreUpdateUnavailable",
                "core-release",
                if error.is_timeout(){"GitHub не ответил за 12 секунд. Встроенные стабильные ядра уже доступны; проверку выпусков можно повторить позже."}else{"Не удалось соединиться с GitHub для проверки выпусков. Проверьте подключение и повторите проверку."},
            )
        })?;
    if !response.status().is_success() {
        return Err(Error::new(
            "CoreUpdateUnavailable",
            "core-release",
            format!("GitHub HTTP {}", response.status().as_u16()),
        ));
    }
    let mut bytes = Vec::new();
    response
        .take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| Error::io("core-release", e))?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err(invalid());
    }
    serde_json::from_slice(&bytes).map_err(|_| invalid())
}
fn parse(engine: &str, value: &serde_json::Value) -> Result<Release> {
    let tag = value["tag_name"].as_str().ok_or_else(invalid)?;
    let name = if engine == "xray" {
        "Xray-windows-64.zip".into()
    } else {
        format!("sing-box-{}-windows-amd64.zip", tag.trim_start_matches('v'))
    };
    let asset = value["assets"]
        .as_array()
        .ok_or_else(invalid)?
        .iter()
        .find(|a| a["name"] == name)
        .ok_or_else(invalid)?;
    let release = Release {
        engine: engine.into(),
        tag: tag.into(),
        prerelease: value["prerelease"].as_bool().unwrap_or(false),
        page: value["html_url"].as_str().ok_or_else(invalid)?.into(),
        asset: name,
        url: asset["browser_download_url"]
            .as_str()
            .ok_or_else(invalid)?
            .into(),
        archive_sha256: asset["digest"]
            .as_str()
            .and_then(|d| d.strip_prefix("sha256:"))
            .ok_or_else(|| {
                Error::new(
                    "CoreChecksumMissing",
                    "core-release",
                    "Official GitHub asset has no SHA-256 digest",
                )
            })?
            .into(),
    };
    validate(&release)?;
    Ok(release)
}
pub fn list(engine: &str) -> Result<Vec<Release>> {
    use std::{collections::HashMap,sync::{Mutex,OnceLock},time::Instant};
    static CACHE: OnceLock<Mutex<HashMap<String,(Instant,Vec<Release>)>>>=OnceLock::new();
    repo(engine)?;
    let cache=CACHE.get_or_init(||Mutex::new(HashMap::new()));
    if let Ok(saved)=cache.lock() {if let Some((at,items))=saved.get(engine) {if at.elapsed()<Duration::from_secs(300) {return Ok(items.clone());}}}
    let _lookup=release_lookup(engine)?;
    // GitHub embeds every platform's assets in every release. A small recent
    // page plus the stable fallback avoids downloading megabytes of unused data.
    let values=metadata(&format!("https://api.github.com/repos/{}/releases?per_page=4",repo(engine)?))?;
    let mut items=values.as_array().ok_or_else(invalid)?.iter().filter(|v|v["draft"]!=true).filter_map(|v|parse(engine,v).ok()).collect::<Vec<_>>();
    if !items.iter().any(|r|!r.prerelease){
        if let Ok(stable)=metadata(&format!("https://api.github.com/repos/{}/releases/latest",repo(engine)?)).and_then(|v|parse(engine,&v)){
            if !items.iter().any(|r|r.tag==stable.tag){items.push(stable);}
        }
    }
    items.sort_by_key(|r|r.prerelease);
    if items.is_empty() {return Err(Error::new("CoreReleasesUnavailable","core-release","GitHub не вернул выпуски Windows x64 с официальной контрольной суммой. Встроенные стабильные ядра доступны без загрузки."));}
    if let Ok(mut saved)=cache.lock(){saved.insert(engine.into(),(Instant::now(),items.clone()));}
    Ok(items)
}
struct ReleaseLookup(&'static std::sync::atomic::AtomicBool);
impl Drop for ReleaseLookup {fn drop(&mut self){self.0.store(false,std::sync::atomic::Ordering::Release);}}
fn release_lookup(engine:&str)->Result<ReleaseLookup> {
    use std::sync::atomic::{AtomicBool,Ordering};
    static XRAY:AtomicBool=AtomicBool::new(false);static SING:AtomicBool=AtomicBool::new(false);
    let flag=if engine=="xray"{&XRAY}else if engine=="sing-box"{&SING}else{return Err(invalid());};
    flag.compare_exchange(false,true,Ordering::AcqRel,Ordering::Acquire).map_err(|_|Error::new("CoreUpdateBusy","core-release","Проверка этого ядра ещё завершается. Дождитесь результата перед повторной попыткой."))?;
    Ok(ReleaseLookup(flag))
}
fn authenticate(release: &Release) -> Result<()> {
    validate(release)?;
    let fresh = parse(
        &release.engine,
        &metadata(&format!(
            "https://api.github.com/repos/{}/releases/tags/{}",
            repo(&release.engine)?,
            release.tag
        ))?,
    )?;
    if fresh.url != release.url || fresh.archive_sha256 != release.archive_sha256 {
        return Err(Error::new(
            "CoreReleaseChanged",
            "core-update",
            "Official release changed; refresh the release list",
        ));
    }
    Ok(())
}
pub fn hash(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path).map_err(|e| crate::engine::component_access_error(&path.to_string_lossy(), e))?;
    let mut sha = Sha256::new();
    let mut bytes = [0u8; 65536];
    loop {
        let n = file
            .read(&mut bytes)
            .map_err(|e| Error::io("core-hash", e))?;
        if n == 0 {
            break;
        }
        sha.update(&bytes[..n]);
    }
    Ok(hex(sha.finalize()))
}
fn archive(release: &Release, cache: Option<&Path>) -> Result<Vec<u8>> {
    let cached = cache.map(|c| c.join(format!("{}.zip", release.archive_sha256)));
    if let Some(cached) = &cached
        && cached.is_file()
        && let Ok(bytes) = std::fs::read(cached)
        && bytes.len() <= 160 * 1024 * 1024
        && hex(Sha256::digest(&bytes)) == release.archive_sha256
    {
        return Ok(bytes);
    }
    let response = crate::network::update_client(Duration::from_secs(120))?
        .get(&release.url)
        .send()
        .map_err(|error| {
            Error::new(
                "CoreDownloadFailed",
                "core-download",
                format!("Не удалось скачать {} {}: {}. Встроенный стабильный комплект можно восстановить без интернета.",release.engine,release.tag,if error.is_timeout(){"сервер не ответил вовремя"}else{"ошибка подключения к официальному серверу"}),
            )
        })?;
    if !response.status().is_success() {
        return Err(Error::new(
            "CoreDownloadFailed",
            "core-download",
            format!("Download HTTP {}", response.status().as_u16()),
        ));
    }
    let mut bytes = Vec::new();
    response
        .take(160 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| Error::io("core-download", e))?;
    if bytes.len() > 160 * 1024 * 1024 || hex(Sha256::digest(&bytes)) != release.archive_sha256 {
        return Err(Error::new(
            "CoreHashMismatch",
            "core-download",
            "Downloaded archive does not match official GitHub SHA-256",
        ));
    }
    if let Some(cache) = cache {
        std::fs::create_dir_all(cache).map_err(|e| Error::io("core-cache", e))?;
        std::fs::write(cached.unwrap(), &bytes).map_err(|e| Error::io("core-cache", e))?;
    }
    Ok(bytes)
}
fn component(bytes: &[u8], engine: &str, name: &str) -> Result<Option<Vec<u8>>> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|_| invalid())?;
    let expected = if engine == "xray" {
        name.to_owned()
    } else {
        let prefix = archive
            .file_names()
            .filter_map(|name| name.ok())
            .find(|n| n.ends_with("/sing-box.exe"))
            .and_then(|n| n.rsplit_once('/').map(|v| v.0.to_owned()))
            .ok_or_else(invalid)?;
        format!("{prefix}/{name}")
    };
    let mut file = match archive.by_name(&expected) {
        Ok(file) => file,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(_) => return Err(invalid()),
    };
    if file.size() > 160 * 1024 * 1024 {
        return Err(invalid());
    }
    let mut result = Vec::new();
    file.read_to_end(&mut result)
        .map_err(|e| Error::io("core-extract", e))?;
    Ok(Some(result))
}
pub fn approval(directory: &Path, engine: &str) -> Result<Option<Approval>> {
    let file = directory.join(manifest_name(engine));
    if !file.exists() {
        return Ok(None);
    }
    let bytes = std::fs::read(file).map_err(|e| Error::io("core-approval", e))?;
    if bytes.len() > 16384 {
        return Err(invalid());
    }
    let record: Approval = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    validate(&record.release)?;
    if record.release.engine != engine
        || !valid_sha(&record.exe_sha256)
        || record.dll_sha256.as_ref().is_some_and(|h| !valid_sha(h))
    {
        return Err(invalid());
    }
    Ok(Some(record))
}
pub fn verify_custom(path: &Path, engine: &str) -> Result<bool> {
    let Some(record) = approval(path.parent().ok_or_else(invalid)?, engine)? else {
        return Ok(false);
    };
    if hash(path)? != record.exe_sha256 {
        return Err(Error::new(
            "CoreHashMismatch",
            "core-verify",
            "Installed engine hash changed",
        ));
    }
    if engine == "sing-box"
        && record.dll_sha256.is_none()
        && hash(&path.with_file_name("libcronet.dll"))? != crate::engine::CRONET_SHA256
    {
        return Err(Error::new(
            "CoreHashMismatch",
            "core-verify",
            "An unapproved engine DLL cannot be loaded",
        ));
    }
    if let Some(expected) = record.dll_sha256
        && hash(&path.with_file_name("libcronet.dll"))? != expected
    {
        return Err(Error::new(
            "CoreHashMismatch",
            "core-verify",
            "Installed engine DLL hash changed",
        ));
    }
    Ok(true)
}
pub fn version(directory: &Path, engine: &str) -> String {
    if let Ok(Some(record))=approval(directory,engine) {return record.release.tag.trim_start_matches('v').into();}
    let path=directory.join(if engine=="xray" {"xray.exe"} else {"sing-box.exe"});
    let Ok(digest)=hash(&path) else {return "—".into();};
    let known=if engine=="xray" {vec![(crate::runtime::XRAY_SHA256,crate::runtime::XRAY_VERSION),("a743b8c2ce49cb33d6c1cf764c780bf1a475d746ad7485e7c3fcfe6458511c92","26.10.17"),("15c2d007954ac53ba69b80ec91242786b3c0b71d52649165b4ca1d5cc96ef8f1","26.3.27")]} else {vec![(crate::engine::ENGINE_SHA256,crate::engine::ENGINE_VERSION),("8f30682ee198855440d2a7fe070d5d3653793b2d01bb8c85eea88f93bbe7c8b9","1.14.4"),("5c19bb3f8c12d4e85b3d47fe63123a7624b314b58dd7a8e32f51004f1c4d2faf","1.14.3")]};
    known.iter().find(|(sha,_)|*sha==digest).map(|(_,v)|v.to_string()).unwrap_or_else(||"—".into())
}
pub fn authenticated_approvals(directory: &Path) -> Result<Vec<Approval>> {
    let mut result = Vec::new();
    for engine in ["sing-box", "xray"] {
        if let Some(record) = approval(directory, engine)? {
            authenticate(&record.release)?;
            let bytes = archive(&record.release, None)?;
            let exe = component(
                &bytes,
                engine,
                if engine == "xray" {
                    "xray.exe"
                } else {
                    "sing-box.exe"
                },
            )?
            .ok_or_else(invalid)?;
            if hex(Sha256::digest(&exe)) != record.exe_sha256 {
                return Err(Error::new(
                    "CoreApprovalMismatch",
                    "core-install",
                    "Executable is not the official archive component",
                ));
            }
            if let Some(expected) = &record.dll_sha256 {
                let dll = component(&bytes, engine, "libcronet.dll")?.ok_or_else(invalid)?;
                if hex(Sha256::digest(&dll)) != *expected {
                    return Err(invalid());
                }
            }
            result.push(record);
        }
    }
    Ok(result)
}
pub fn write_approvals(directory: &Path, records: &[Approval]) -> Result<()> {
    for engine in ["sing-box", "xray"] {
        let path = directory.join(manifest_name(engine));
        if let Some(record) = records.iter().find(|r| r.release.engine == engine) {
            std::fs::write(path, serde_json::to_vec(record).map_err(|_| invalid())?)
                .map_err(|e| Error::io("core-approval-save", e))?;
        } else if path.exists() {
            std::fs::remove_file(path).map_err(|e| Error::io("core-approval-save", e))?;
        }
    }
    Ok(())
}
fn fill_missing_companions(stage: &Path, bundle: &Path) -> Result<()> {
    crate::engine::verify_engine(&bundle.join("sing-box.exe"))?;
    crate::runtime::verify_xray(&bundle.join("xray.exe"))?;
    for name in ["sing-box.exe","libcronet.dll","xray.exe"] {
        match std::fs::symlink_metadata(stage.join(name)) {
            Ok(_) => {},
            Err(e) if e.kind()==std::io::ErrorKind::NotFound => {
                std::fs::copy(bundle.join(name),stage.join(name)).map_err(|e|crate::engine::component_access_error(&bundle.join(name).to_string_lossy(),e))?;
            },
            Err(e)=>return Err(crate::engine::component_access_error(&stage.join(name).to_string_lossy(),e)),
        }
    }
    Ok(())
}
pub fn install(
    release: Release,
    data: &Path,
    profiles: &[crate::model::Profile],
    prefs: &crate::model::Preferences,
) -> Result<()> {
    install_with_bundle(release,data,profiles,prefs,None)
}
pub fn install_with_bundle(
    release: Release,
    data: &Path,
    profiles: &[crate::model::Profile],
    prefs: &crate::model::Preferences,
    bundle: Option<&Path>,
) -> Result<()> {
    authenticate(&release)?;
    let target = data.join("engine");
    let stage = data.join(format!("core-stage-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&stage).map_err(|e| Error::io("core-stage", e))?;
    let result = (|| {
        for name in [
            "sing-box.exe",
            "libcronet.dll",
            "xray.exe",
            "sing-box-release.json",
            "xray-release.json",
        ] {
            if target.join(name).is_file() {
                std::fs::copy(target.join(name), stage.join(name))
                    .map_err(|e| crate::engine::component_access_error(&target.join(name).to_string_lossy(),e))?;
            }
        }
        // This explicit update may follow a failed/missing runtime installation.
        // Fill absent companion components from the shipped, verified bundle;
        // preserve all present components, including user-selected versions.
        if let Some(bundle)=bundle {fill_missing_companions(&stage,bundle)?;}
        let bytes = archive(&release, Some(&data.join("core-downloads")))?;
        let name = if release.engine == "xray" {
            "xray.exe"
        } else {
            "sing-box.exe"
        };
        let exe = component(&bytes, &release.engine, name)?.ok_or_else(invalid)?;
        std::fs::write(stage.join(name), &exe).map_err(|e| Error::io("core-stage", e))?;
        let dll = if release.engine == "sing-box" {
            component(&bytes, &release.engine, "libcronet.dll")?
        } else {
            None
        };
        if let Some(dll) = &dll {
            std::fs::write(stage.join("libcronet.dll"), dll)
                .map_err(|e| Error::io("core-stage", e))?;
        }
        let record = Approval {
            release: release.clone(),
            exe_sha256: hex(Sha256::digest(&exe)),
            dll_sha256: dll.map(|d| hex(Sha256::digest(&d))),
        };
        std::fs::write(
            stage.join(manifest_name(&release.engine)),
            serde_json::to_vec(&record).map_err(|_| invalid())?,
        )
        .map_err(|e| Error::io("core-stage", e))?;
        crate::engine::verify_engine(&stage.join("sing-box.exe"))?;
        crate::runtime::verify_xray(&stage.join("xray.exe"))?;
        let actual = executable_version(&stage.join(name), &release.engine)?;
        if actual != release.tag.trim_start_matches('v') {
            return Err(Error::new(
                "CoreVersionMismatch",
                "core-update",
                "Версия файла не совпала с выбранным релизом",
            ));
        }
        let mut checked = prefs.clone();
        checked.tun = false;
        checked.system_proxy = false;
        checked.route_mode = crate::model::RouteMode::Global;
        let fallback = crate::imports::parse_uri(
            "vless://15bc786a-b01b-4a8a-962e-cb52e089e500@example.org:443?security=tls#CompatibilityCheck",
        )?;
        checked.transport_engine = "xray".into();
        let mut candidates = vec![fallback];
        candidates.extend_from_slice(profiles);

        for (index, profile) in candidates.iter().take(201).enumerate() {
            for choice in ["auto", "xray"] {
                checked.transport_engine = choice.into();
                // A Hysteria2 profile belongs to sing-box. Keeping it in a
                // subscription must not prevent an independent Xray update.
                let resolved=profile.resolved_with(&checked)?;
                if crate::transport_options::use_xray(&resolved,&checked).is_err() {continue;}
                crate::runtime::RuntimePlan::new(
                    &stage.join("sing-box.exe"),
                    profile,
                    &checked,
                    crate::runtime::free_port()?,
                    "compatibility-api",
                    "compatibility-proxy",
                    [crate::runtime::free_port()?, crate::runtime::free_port()?],
                )?
                .validate(&stage.join("sing-box.exe"))?;
            }
            if index == 0 {
                let mut tun = checked.clone();
                tun.tun = true;
                tun.route_mode = prefs.route_mode.clone();
                crate::runtime::RuntimePlan::new(
                    &stage.join("sing-box.exe"),
                    profile,
                    &tun,
                    crate::runtime::free_port()?,
                    "compatibility-api",
                    "compatibility-proxy",
                    [crate::runtime::free_port()?, crate::runtime::free_port()?],
                )?
                .validate(&stage.join("sing-box.exe"))?;
            }
        }
        crate::core_updates::activate_staged(&stage, data)?;
        if hash(&target.join(name))? != record.exe_sha256 {
            return Err(Error::new(
                "CoreActivationMismatch",
                "core-update",
                "Контрольная сумма активированного ядра не совпала",
            ));
        }
        // Service installation re-downloads and authenticates archives independently;
        // keeping a ZIP after activation is unnecessary. Previous binaries stay for rollback.
        let _ = cleanup_downloads(&data.join("core-downloads"));
        Ok(())
    })();
    if stage.exists() {
        for name in [
            "sing-box.exe",
            "libcronet.dll",
            "xray.exe",
            "sing-box-release.json",
            "xray-release.json",
        ] {
            let _ = std::fs::remove_file(stage.join(name));
        }
        let _ = std::fs::remove_dir(stage);
    }
    let _ = cleanup_downloads(&data.join("core-downloads"));
    result
}
pub fn cleanup_stages(data:&Path)->Result<usize> {
 crate::layout::normal_directory(data)?;
 let mut count=0;
 for item in std::fs::read_dir(data).map_err(|e|Error::io("core-stage-clean",e))? {
  let item=item.map_err(|e|Error::io("core-stage-clean",e))?;
  let name=item.file_name().to_string_lossy().into_owned();
  if !item.file_type().is_ok_and(|t|t.is_dir()) || !name.strip_prefix("core-stage-").is_some_and(|s|uuid::Uuid::parse_str(s).is_ok()){continue}
  if crate::layout::normal_directory(&item.path()).is_err(){continue;}
  for name in ["sing-box.exe","libcronet.dll","xray.exe","sing-box-release.json","xray-release.json"] {
   let path=item.path().join(name);
   if path.symlink_metadata().is_ok_and(|m|m.file_type().is_file()){std::fs::remove_file(path).map_err(|e|Error::io("core-stage-clean",e))?;count+=1}
  }
  let _=std::fs::remove_dir(item.path());
 }
 Ok(count)
}
pub fn cleanup_downloads(directory: &Path) -> Result<usize> {
    crate::layout::normal_directory(directory)?;
    if !directory.exists() {
        return Ok(0);
    }
    let mut removed = 0;
    for item in std::fs::read_dir(directory).map_err(|e| Error::io("core-cache-clean", e))? {
        let item = item.map_err(|e| Error::io("core-cache-clean", e))?;
        let path = item.path();
        if item
            .file_type()
            .map_err(|e| Error::io("core-cache-clean", e))?
            .is_file()
            && path.extension().is_some_and(|x| x == "zip")
            && path
                .file_stem()
                .and_then(|s| s.to_str())
                .is_some_and(valid_sha)
        {
            std::fs::remove_file(path).map_err(|e| Error::io("core-cache-clean", e))?;
            removed += 1;
        }
    }
    Ok(removed)
}
pub fn executable_version(path: &Path, engine: &str) -> Result<String> {
    use std::process::{Command, Stdio};
    let mut cmd = Command::new(path);
    cmd.arg("version")
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    let mut child = cmd.spawn().map_err(|e| crate::engine::component_access_error(&path.to_string_lossy(),e))?;
    let until = std::time::Instant::now() + Duration::from_secs(4);
    loop {
        if let Some(status) = child.try_wait().map_err(|e| Error::io("core-version", e))? {
            if !status.success() {
                return Err(invalid());
            }
            break;
        }
        if std::time::Instant::now() >= until {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error::new(
                "CoreVersionTimeout",
                "core-version",
                "Ядро не ответило на запрос версии",
            ));
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    let mut text = String::new();
    child
        .stdout
        .take()
        .ok_or_else(invalid)?
        .take(4096)
        .read_to_string(&mut text)
        .map_err(|e| Error::io("core-version", e))?;
    let prefix = if engine == "xray" {
        "Xray "
    } else {
        "sing-box version "
    };
    text.lines()
        .find_map(|line| line.strip_prefix(prefix))
        .and_then(|s| s.split_whitespace().next())
        .map(str::to_owned)
        .ok_or_else(invalid)
}
#[cfg(test)]
mod tests {
    #[test] fn only_one_release_lookup_per_engine_can_remain_in_flight() {
        let first=super::release_lookup("sing-box").unwrap();assert!(super::release_lookup("sing-box").is_err());
        let other=super::release_lookup("xray").unwrap();drop(first);assert!(super::release_lookup("sing-box").is_ok());drop(other);
    }
    #[test] fn cleanup_cannot_follow_junctions_outside_its_cache() {
        use std::os::windows::process::CommandExt;
        let d=tempfile::tempdir().unwrap();let target=d.path().join("outside");std::fs::create_dir(&target).unwrap();
        let zip=target.join(format!("{}.zip","a".repeat(64)));std::fs::write(&zip,b"preserve").unwrap();
        std::fs::write(target.join("sing-box.exe"),b"preserve").unwrap();
        let data=d.path().join("data");std::fs::create_dir(&data).unwrap();
        let stage=data.join(format!("core-stage-{}",uuid::Uuid::new_v4()));
        for link in [&stage,&data.join("updates")] {
            let made=std::process::Command::new("cmd.exe").args(["/c","mklink","/J"]).arg(link).arg(&target).creation_flags(0x08000000).output().unwrap();assert!(made.status.success());
        }
        assert!(super::cleanup_downloads(&stage).is_err());assert_eq!(super::cleanup_stages(&data).unwrap(),0);assert_eq!(crate::client_updates::cleanup(&data,"1.0.6").unwrap(),0);
        assert_eq!(std::fs::read(zip).unwrap(),b"preserve");assert_eq!(std::fs::read(target.join("sing-box.exe")).unwrap(),b"preserve");
        std::fs::remove_dir(stage).unwrap();std::fs::remove_dir(data.join("updates")).unwrap();
    }
    #[test]
    #[ignore = "Pinned local runtime fixtures only; no downloads or system changes"]
    fn explicit_install_fills_missing_companion_without_overwriting_existing_files() {
        let stage=tempfile::tempdir().unwrap();let bundle=tempfile::tempdir().unwrap();
        let sbox=std::path::PathBuf::from(std::env::var_os("NETPULSE_TEST_ENGINE").unwrap());let xray=std::path::PathBuf::from(std::env::var_os("NETPULSE_TEST_XRAY").unwrap());
        for (source,name) in [(sbox.clone(),"sing-box.exe"),(sbox.with_file_name("libcronet.dll"),"libcronet.dll"),(xray,"xray.exe")] {std::fs::copy(source,bundle.path().join(name)).unwrap();}
        std::fs::write(stage.path().join("xray.exe"),b"existing selected version").unwrap();
        super::fill_missing_companions(stage.path(),bundle.path()).unwrap();
        crate::engine::verify_engine(&stage.path().join("sing-box.exe")).unwrap();assert_eq!(std::fs::read(stage.path().join("xray.exe")).unwrap(),b"existing selected version");
        std::fs::write(bundle.path().join("sing-box.exe"),b"tampered").unwrap();assert!(super::fill_missing_companions(stage.path(),bundle.path()).is_err());
    }
    use super::*;
    #[test]
    fn cache_cleanup_removes_only_owned_zip_names() {
        let d = tempfile::tempdir().unwrap();
        let zip = d.path().join(format!("{}.zip", "a".repeat(64)));
        std::fs::write(&zip, b"download").unwrap();
        std::fs::write(d.path().join("keep.zip"), b"unrelated").unwrap();
        std::fs::create_dir(d.path().join(format!("{}.zip", "b".repeat(64)))).unwrap();
        assert_eq!(cleanup_downloads(d.path()).unwrap(), 1);
        assert!(!zip.exists());
        assert!(d.path().join("keep.zip").is_file());
    }
    #[test]
    #[ignore = "Requires pinned official executables"]
    fn actual_executables_report_their_versions() {
        let sing = std::path::PathBuf::from(std::env::var("NETPULSE_TEST_ENGINE").unwrap());
        let xray = std::path::PathBuf::from(std::env::var("NETPULSE_TEST_XRAY").unwrap());
        assert_eq!(
            executable_version(&sing, "sing-box").unwrap(),
            crate::engine::ENGINE_VERSION
        );
        assert_eq!(
            executable_version(&xray, "xray").unwrap(),
            crate::runtime::XRAY_VERSION
        );
    }
    #[test]
    fn reject_untrusted_release_urls_and_hashes() {
        let r = Release {
            engine: "xray".into(),
            tag: "v26.10.17".into(),
            prerelease: true,
            page: "https://github.com/XTLS/Xray-core/releases/tag/v26.10.17".into(),
            asset: "Xray-windows-64.zip".into(),
            url: "https://evil.example/core.zip".into(),
            archive_sha256: "0".repeat(64),
        };
        assert!(validate(&r).is_err());
        let mut r = r;
        r.url = "https://github.com/XTLS/Xray-core/releases/download/v26.10.17/Xray-windows-64.zip"
            .into();
        assert!(validate(&r).is_ok());
        r.tag = "../bad".into();
        assert!(validate(&r).is_err());
    }
    #[test]
    fn custom_approval_detects_file_and_manifest_tampering() {
        let data = tempfile::tempdir().unwrap();
        let exe = data.path().join("xray.exe");
        std::fs::write(&exe, b"fixture").unwrap();
        let release = Release {
            engine: "xray".into(),
            tag: "v26.10.17".into(),
            prerelease: true,
            page: "https://github.com/XTLS/Xray-core/releases/tag/v26.10.17".into(),
            asset: "Xray-windows-64.zip".into(),
            url:
                "https://github.com/XTLS/Xray-core/releases/download/v26.10.17/Xray-windows-64.zip"
                    .into(),
            archive_sha256: "0".repeat(64),
        };
        let record = Approval {
            release,
            exe_sha256: hash(&exe).unwrap(),
            dll_sha256: None,
        };
        write_approvals(data.path(), std::slice::from_ref(&record)).unwrap();
        assert!(verify_custom(&exe, "xray").unwrap());
        std::fs::write(&exe, b"tampered").unwrap();
        assert_eq!(
            verify_custom(&exe, "xray").unwrap_err().code,
            "CoreHashMismatch"
        );
        let mut invalid = record;
        invalid.release.url = "file:///fixture".into();
        std::fs::write(
            data.path().join(manifest_name("xray")),
            serde_json::to_vec(&invalid).unwrap(),
        )
        .unwrap();
        assert!(approval(data.path(), "xray").is_err());
    }
}
