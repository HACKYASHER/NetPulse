use crate::{
    Error, Result,
    imports::{ImportReport, Parsed},
    model::{Preferences, Profile, ProfileSummary},
    vault,
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
use zeroize::Zeroizing;

fn db(e: rusqlite::Error) -> Error {
    Error::new(
        "StorageError",
        "sqlite",
        match e {
            rusqlite::Error::SqliteFailure(ref err, _) => {
                format!("SQLite {:?} ({})", err.code, err.extended_code)
            }
            _ => "SQLite operation failed".into(),
        },
    )
}
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LogEvent {
    pub id: String,
    pub timestamp: u64,
    pub level: String,
    pub subsystem: String,
    pub stage: String,
    pub code: String,
    pub message: String,
}
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct SubscriptionUsage {
    pub upload: Option<u64>,
    pub download: Option<u64>,
    pub total: Option<u64>,
    pub expire: Option<u64>,
}
impl SubscriptionUsage {
    /// Provider metadata is optional. Reject duplicates, signed/overflowing
    /// numbers and oversized input without logging the private response header.
    pub fn parse(header: &str) -> Option<Self> {
        if header.len() > 1024 {
            return None;
        }
        let mut v = Self::default();
        for part in header.split(';').filter(|s| !s.trim().is_empty()) {
            let (key, raw) = part.trim().split_once('=')?;
            let raw = raw.trim();
            if raw.is_empty() || !raw.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            let n = raw
                .parse::<u64>()
                .ok()
                .filter(|n| *n <= 9_007_199_254_740_991)?;
            let field = match key.trim().to_ascii_lowercase().as_str() {
                "upload" => &mut v.upload,
                "download" => &mut v.download,
                "total" => &mut v.total,
                "expire" => &mut v.expire,
                _ => continue,
            };
            if field.replace(n).is_some() {
                return None;
            }
        }
        if v == Self::default() { None } else { Some(v) }
    }
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SubscriptionSummary {
    pub id: String,
    pub name: String,
    pub updated_at: u64,
    pub count: usize,
    pub interval_hours: u32,
    #[serde(default = "protected_by_default")]
    pub url_protected: bool,
    #[serde(default)]
    pub config_locked: bool,
    #[serde(default)]
    pub usage: Option<SubscriptionUsage>,
}
impl SubscriptionSummary {
    pub fn due(&self, now: u64) -> bool {
        self.updated_at == 0
            || now.saturating_sub(self.updated_at) >= u64::from(self.interval_hours.max(1)) * 3600
    }
}
fn protected_by_default() -> bool {
    true
}
pub struct Store {
    db: Connection,
    directory: std::path::PathBuf,
}
impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::io("storage-directory", e))?;
        }
        let mut db_ = Connection::open(path).map_err(db)?;
        db_.busy_timeout(std::time::Duration::from_secs(3))
            .map_err(db)?;
        let version: i64 = db_
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(db)?;
        if version > 4 {
            return Err(Error::new(
                "SchemaTooNew",
                "storage-open",
                "This database was created by a newer NetPulse version",
            ));
        }
        db_.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; PRAGMA secure_delete=ON;",
        )
        .map_err(db)?;
        if version > 0 && version < 4 {
            let raw = db_.serialize("main").map_err(db)?;
            let sealed = vault::seal(raw.as_ref())?;
            let backup = path.with_extension(format!("before-v4-{}.npdb", uuid::Uuid::new_v4()));
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(backup)
                .map_err(|e| Error::io("migration-backup", e))?;
            file.write_all(&sealed)
                .and_then(|_| file.sync_all())
                .map_err(|e| Error::io("migration-backup", e))?;
        }
        let tx = db_.transaction().map_err(db)?;
        tx.execute_batch("CREATE TABLE IF NOT EXISTS profiles(id TEXT PRIMARY KEY, fingerprint TEXT NOT NULL, subscription_id TEXT, sealed BLOB NOT NULL);
          CREATE UNIQUE INDEX IF NOT EXISTS profile_source ON profiles(fingerprint,COALESCE(subscription_id,''));
          CREATE TABLE IF NOT EXISTS settings(id INTEGER PRIMARY KEY CHECK(id=1), value TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS subscriptions(id TEXT PRIMARY KEY,name TEXT NOT NULL,sealed_url BLOB NOT NULL,updated_at INTEGER NOT NULL DEFAULT 0,interval_hours INTEGER NOT NULL DEFAULT 24);
          CREATE TABLE IF NOT EXISTS events(id TEXT PRIMARY KEY,timestamp INTEGER NOT NULL,level TEXT NOT NULL,subsystem TEXT NOT NULL,stage TEXT NOT NULL,code TEXT NOT NULL,message TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS measurements(id INTEGER PRIMARY KEY AUTOINCREMENT,kind TEXT NOT NULL,timestamp INTEGER NOT NULL,sealed BLOB NOT NULL);
          PRAGMA user_version=4;").map_err(db)?;
        let has_protection: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM pragma_table_info('subscriptions') WHERE name='url_protected')", [], |r| r.get(0)).map_err(db)?;
        if !has_protection {
            tx.execute_batch(
                "ALTER TABLE subscriptions ADD COLUMN url_protected INTEGER NOT NULL DEFAULT 1;",
            )
            .map_err(db)?;
        }
        let has_usage: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM pragma_table_info('subscriptions') WHERE name='usage')", [], |r| r.get(0)).map_err(db)?;
        if !has_usage {
            tx.execute_batch("ALTER TABLE subscriptions ADD COLUMN usage TEXT;")
                .map_err(db)?;
        }
        // Tombstones survive deletion and restoring an older backup.
        tx.execute_batch("CREATE TABLE IF NOT EXISTS subscription_locks(id TEXT PRIMARY KEY);")
            .map_err(db)?;
        tx.commit().map_err(db)?;
        Ok(Self {
            db: db_,
            directory: path.parent().unwrap_or(Path::new(".")).to_owned(),
        })
    }
    pub fn data_directory(&self) -> &Path {
        &self.directory
    }
    #[cfg(test)]
    pub(crate) fn test_read_only(&self, enabled: bool) {
        self.db
            .execute_batch(if enabled {
                "PRAGMA query_only=ON"
            } else {
                "PRAGMA query_only=OFF"
            })
            .unwrap();
    }
    pub fn preferences(&self) -> Result<Preferences> {
        let raw: Option<String> = self
            .db
            .query_row("SELECT value FROM settings WHERE id=1", [], |r| r.get(0))
            .optional()
            .map_err(db)?;
        let value = match raw {
            Some(raw) => serde_json::from_str(&raw).map_err(|_| {
                Error::new("StorageError", "settings-read", "Invalid settings document")
            })?,
            None => Preferences::default(),
        };
        value.validate()?;
        Ok(value)
    }
    pub fn save_preferences(&self, prefs: &Preferences) -> Result<()> {
        let prefs = prefs.clone().normalized();
        prefs.validate()?;
        self.db.execute("INSERT INTO settings VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET value=excluded.value",[serde_json::to_string(&prefs).unwrap()]).map_err(db)?;
        let _=self.log("INFO","Settings","preferences","Saved",&format!("TUN={} SystemProxy={} routing={:?} MTU={} DNS={} engine={} applications={} autostart={} close_to_tray={} minimize_to_tray={} auto_failover={} theme={}",prefs.tun,prefs.system_proxy,prefs.route_mode,prefs.mtu,prefs.dns_route,prefs.transport_engine,prefs.applications.len(),prefs.autostart,prefs.close_to_tray,prefs.minimize_to_tray,prefs.auto_failover,prefs.theme));
        Ok(())
    }
    pub fn profile(&self, id: &str) -> Result<Profile> {
        let sealed: Vec<u8> = self
            .db
            .query_row("SELECT sealed FROM profiles WHERE id=?1", [id], |r| {
                r.get(0)
            })
            .map_err(|e| {
                if matches!(e, rusqlite::Error::QueryReturnedNoRows) {
                    Error::new("ProfileNotFound", "profile-load", "Profile does not exist")
                } else {
                    db(e)
                }
            })?;
        let bytes = vault::open(&sealed)?;
        let p: Profile = serde_json::from_slice(&bytes).map_err(|_| {
            Error::new(
                "StorageError",
                "profile-load",
                "Stored profile could not be decoded",
            )
        })?;
        p.validate()?;
        Ok(p)
    }
    pub fn profiles(&self) -> Result<Vec<ProfileSummary>> {
        let mut stmt = self
            .db
            .prepare("SELECT id FROM profiles ORDER BY rowid")
            .map_err(db)?;
        let ids = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(db)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db)?;
        let measurements = self.measurements("latency")?;
        ids.iter()
            .map(|id| {
                self.profile(id).map(|p| {
                    let mut summary = p.summary();
                    if let Some(v) = measurements.iter().find(|v| {
                        v["profile_id"].as_str() == Some(id.as_str())
                            && v["method"]
                                == self
                                    .preferences()
                                    .map(|p| p.ping_method)
                                    .unwrap_or_else(|_| "proxy_https_rtt".into())
                    }) {
                        summary.latency_ms = v["min_ms"]
                            .as_f64()
                            .or_else(|| v["avg_ms"].as_f64())
                            .map(|v| v.round() as u64);
                        summary.latency_method = v["method"].as_str().map(str::to_owned);
                        summary.latency_at = v["timestamp"].as_u64();
                        summary.latency_status = Some(
                            if summary.latency_ms.is_some() {
                                "success"
                            } else {
                                "failed"
                            }
                            .into(),
                        );
                    }
                    summary
                })
            })
            .collect()
    }
    pub fn import(
        &mut self,
        mut parsed: Parsed,
        subscription: Option<&str>,
    ) -> Result<ImportReport> {
        // Build and encrypt everything before beginning a mutation.
        let mut rows = vec![];
        for p in &mut parsed.profiles {
            p.subscription_id = subscription.map(str::to_string);
            p.validate()?;
            let fingerprint = Sha256::digest(serde_json::to_vec(&p.outbound).unwrap())
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>();
            let previous:Option<String>=self.db.query_row("SELECT id FROM profiles WHERE fingerprint=?1 AND COALESCE(subscription_id,'')=COALESCE(?2,'')",params![fingerprint,subscription],|r|r.get(0)).optional().map_err(db)?;
            if let Some(id) = previous {
                let old = self.profile(&id)?;
                p.transport_override = old.transport_override.clone();
                p.outbound_override = old.outbound_override.clone();
                p.validate()?;
                p.id = id;
            }
            let raw = Zeroizing::new(serde_json::to_vec(p).unwrap());
            let sealed = vault::seal(&raw)?;
            rows.push((p.id.clone(), fingerprint, sealed));
        }
        let tx = self.db.transaction().map_err(db)?;
        // Preserve previous good nodes after a partial update. Complete updates replace.
        if let Some(id) = subscription
            && parsed.report.issues.is_empty()
        {
            tx.execute("DELETE FROM profiles WHERE subscription_id=?1", [id])
                .map_err(db)?;
        }
        let mut inserted = 0;
        for (id, fp, sealed) in rows {
            inserted+=tx.execute("INSERT OR IGNORE INTO profiles(id,fingerprint,subscription_id,sealed) VALUES(?1,?2,?3,?4)",params![id,fp,subscription,sealed]).map_err(db)?;
        }
        if let Some(id) = subscription {
            tx.execute(
                "UPDATE subscriptions SET updated_at=?1 WHERE id=?2",
                params![now() as i64, id],
            )
            .map_err(db)?;
        }
        if let Some(id) = subscription {
            tx.execute(
                "UPDATE subscriptions SET usage=?1 WHERE id=?2",
                params![
                    parsed
                        .usage
                        .as_ref()
                        .map(|u| serde_json::to_string(u).unwrap()),
                    id
                ],
            )
            .map_err(db)?;
        }
        tx.commit().map_err(db)?;
        parsed.report.imported = inserted;
        Ok(parsed.report)
    }
    pub fn delete_profile(&self, id: &str) -> Result<()> {
        self.db
            .execute("DELETE FROM profiles WHERE id=?1", [id])
            .map_err(db)?;
        Ok(())
    }
    pub fn ensure_profile_editable(&self, id: &str) -> Result<()> {
        let sub: Option<String> = self
            .db
            .query_row(
                "SELECT subscription_id FROM profiles WHERE id=?1",
                [id],
                |r| r.get(0),
            )
            .map_err(db)?;
        if let Some(sub) = sub {
            let locked: bool = self
                .db
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM subscription_locks WHERE id=?1)",
                    [sub],
                    |r| r.get(0),
                )
                .map_err(db)?;
            if locked {
                return Err(Error::new(
                    "ConfigurationLocked",
                    "profile-json",
                    "Редактирование конфигураций этой подписки запрещено без возможности отмены",
                ));
            }
        }
        Ok(())
    }
    pub fn lock_subscription(&self, id: &str) -> Result<()> {
        let n = self
            .db
            .execute(
                "INSERT OR IGNORE INTO subscription_locks SELECT id FROM subscriptions WHERE id=?1",
                [id],
            )
            .map_err(db)?;
        if n > 0 {
            self.log(
                "INFO",
                "Subscriptions",
                "subscription-lock",
                "Locked",
                "Subscription configuration editing permanently disabled",
            )?;
        }
        Ok(())
    }
    pub fn save_profile_transport(&self, p: &Profile) -> Result<()> {
        self.ensure_profile_editable(&p.id)?;
        p.validate()?;
        let raw = Zeroizing::new(serde_json::to_vec(p).unwrap());
        let n = self
            .db
            .execute(
                "UPDATE profiles SET sealed=?1 WHERE id=?2",
                params![vault::seal(&raw)?, p.id],
            )
            .map_err(db)?;
        if n != 1 {
            return Err(Error::new(
                "ProfileNotFound",
                "profile-transport",
                "Профиль удалён",
            ));
        }
        Ok(())
    }
    pub fn rename_profile(&self, id: &str, name: &str) -> Result<()> {
        let mut p = self.profile(id)?;
        p.name = name.into();
        p.validate()?;
        let raw = Zeroizing::new(serde_json::to_vec(&p).unwrap());
        self.db
            .execute(
                "UPDATE profiles SET sealed=?1 WHERE id=?2",
                params![vault::seal(&raw)?, id],
            )
            .map_err(db)?;
        Ok(())
    }
    pub fn add_subscription(&self, name: &str, url: &str) -> Result<String> {
        self.add_subscription_with_protection(name, url, true)
    }
    pub fn add_subscription_with_protection(
        &self,
        name: &str,
        url: &str,
        protected: bool,
    ) -> Result<String> {
        if name.trim().is_empty() || name.len() > 256 || name.chars().any(char::is_control) {
            return Err(Error::new(
                "InvalidSubscription",
                "subscription-add",
                "Invalid subscription name",
            ));
        }
        let u = url::Url::parse(url).map_err(|_| {
            Error::new(
                "InvalidSubscription",
                "subscription-add",
                "Invalid subscription URL",
            )
        })?;
        if u.scheme() != "https"
            || u.host_str().is_none()
            || !u.username().is_empty()
            || u.password().is_some()
        {
            return Err(Error::new(
                "InvalidSubscription",
                "subscription-add",
                "Subscription must use HTTPS without URL userinfo",
            ));
        }
        let id = uuid::Uuid::new_v4().to_string();
        let sealed = vault::seal(url.as_bytes())?;
        self.db
            .execute(
                "INSERT INTO subscriptions(id,name,sealed_url,url_protected) VALUES(?1,?2,?3,?4)",
                params![id, name, sealed, protected],
            )
            .map_err(db)?;
        Ok(id)
    }
    pub fn edit_subscription(
        &self,
        id: &str,
        name: &str,
        interval_hours: u32,
        protected: bool,
    ) -> Result<()> {
        let name = name.trim();
        if name.is_empty()
            || name.len() > 256
            || name.chars().any(char::is_control)
            || !(1..=168).contains(&interval_hours)
        {
            return Err(Error::new(
                "InvalidSubscription",
                "subscription-edit",
                "Укажите название и интервал обновления от 1 до 168 часов",
            ));
        }
        let changed = self
            .db
            .execute(
                "UPDATE subscriptions SET name=?2,interval_hours=?3,url_protected=?4 WHERE id=?1",
                params![id, name, interval_hours, protected],
            )
            .map_err(db)?;
        if changed == 0 {
            return Err(Error::new(
                "SubscriptionMissing",
                "subscription-edit",
                "Подписка не найдена",
            ));
        }
        self.log(
            "INFO",
            "Subscriptions",
            "subscription-edit",
            "Saved",
            "Subscription settings saved",
        )
    }
    pub fn subscription_url(&self, id: &str) -> Result<Zeroizing<String>> {
        let raw: Vec<u8> = self
            .db
            .query_row(
                "SELECT sealed_url FROM subscriptions WHERE id=?1",
                [id],
                |r| r.get(0),
            )
            .map_err(db)?;
        String::from_utf8(vault::open(&raw)?.to_vec())
            .map(Zeroizing::new)
            .map_err(|_| {
                Error::new(
                    "VaultError",
                    "subscription-load",
                    "Invalid encrypted subscription",
                )
            })
    }
    pub fn subscriptions(&self) -> Result<Vec<SubscriptionSummary>> {
        let mut s=self.db.prepare("SELECT s.id,s.name,s.updated_at,(SELECT COUNT(*) FROM profiles p WHERE p.subscription_id=s.id),s.interval_hours,s.url_protected,EXISTS(SELECT 1 FROM subscription_locks l WHERE l.id=s.id),s.usage FROM subscriptions s ORDER BY s.rowid").map_err(db)?;
        s.query_map([], |r| {
            Ok(SubscriptionSummary {
                id: r.get(0)?,
                name: r.get(1)?,
                updated_at: r.get::<_, i64>(2)? as u64,
                count: r.get::<_, i64>(3)? as usize,
                interval_hours: r.get(4)?,
                url_protected: r.get(5)?,
                config_locked: r.get(6)?,
                usage: r
                    .get::<_, Option<String>>(7)?
                    .and_then(|s| serde_json::from_str(&s).ok()),
            })
        })
        .map_err(db)?
        .collect::<std::result::Result<_, _>>()
        .map_err(db)
    }
    pub fn delete_subscription(&mut self, id: &str) -> Result<()> {
        let tx = self.db.transaction().map_err(db)?;
        tx.execute("DELETE FROM profiles WHERE subscription_id=?1", [id])
            .map_err(db)?;
        tx.execute("DELETE FROM subscriptions WHERE id=?1", [id])
            .map_err(db)?;
        tx.commit().map_err(db)
    }
    pub fn copyable_subscription_url(&self, id: &str) -> Result<Zeroizing<String>> {
        let protected: bool = self
            .db
            .query_row(
                "SELECT url_protected FROM subscriptions WHERE id=?1",
                [id],
                |r| r.get(0),
            )
            .map_err(db)?;
        if protected {
            return Err(Error::new(
                "SubscriptionProtected",
                "subscription-copy",
                "Копирование защищённой ссылки отключено",
            ));
        }
        self.subscription_url(id)
    }
    pub fn log(
        &self,
        level: &str,
        subsystem: &str,
        stage: &str,
        code: &str,
        message: &str,
    ) -> Result<()> {
        // Callers pass structured errors only; raw engine output / URIs are never persisted.
        if level == "DEBUG" && subsystem == "Interface" {
            return Ok(());
        }
        if message.contains("://") || message.len() > 2048 {
            return Err(Error::new(
                "UnsafeLog",
                "log-write",
                "Raw URL or oversized message rejected",
            ));
        }
        let event = LogEvent {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: now(),
            level: level.into(),
            subsystem: subsystem.into(),
            stage: stage.into(),
            code: code.into(),
            message: message.into(),
        };
        crate::audit::append(&self.directory, &event)?;
        // Detailed command/UI audit belongs in the file, not the user event feed.
        if level == "DEBUG" && ["Command", "Interface"].contains(&subsystem) {
            return Ok(());
        }
        self.db
            .execute(
                "INSERT INTO events VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![
                    event.id,
                    event.timestamp as i64,
                    level,
                    subsystem,
                    stage,
                    code,
                    message
                ],
            )
            .map_err(db)?;
        self.db.execute("DELETE FROM events WHERE rowid NOT IN (SELECT rowid FROM events ORDER BY rowid DESC LIMIT 20000)",[]).map_err(db)?;
        Ok(())
    }
    pub fn logs(&self) -> Result<Vec<LogEvent>> {
        let mut s=self.db.prepare("SELECT id,timestamp,level,subsystem,stage,code,message FROM events WHERE NOT(level='DEBUG' AND subsystem IN ('Command','Interface')) ORDER BY rowid DESC LIMIT 1000").map_err(db)?;
        s.query_map([], |r| {
            Ok(LogEvent {
                id: r.get(0)?,
                timestamp: r.get::<_, i64>(1)? as u64,
                level: r.get(2)?,
                subsystem: r.get(3)?,
                stage: r.get(4)?,
                code: r.get(5)?,
                message: r.get(6)?,
            })
        })
        .map_err(db)?
        .collect::<std::result::Result<_, _>>()
        .map_err(db)
    }
    pub fn clear_logs(&self) -> Result<()> {
        self.db.execute("DELETE FROM events", []).map_err(db)?;
        Ok(())
    }
    pub fn clear_file_logs(&self) -> Result<()> {
        crate::audit::clear(&self.directory)?;
        self.clear_logs()
    }
    pub fn event(&self, id: &str) -> Result<LogEvent> {
        self.db
            .query_row(
                "SELECT id,timestamp,level,subsystem,stage,code,message FROM events WHERE id=?1",
                [id],
                |r| {
                    Ok(LogEvent {
                        id: r.get(0)?,
                        timestamp: r.get::<_, i64>(1)? as u64,
                        level: r.get(2)?,
                        subsystem: r.get(3)?,
                        stage: r.get(4)?,
                        code: r.get(5)?,
                        message: r.get(6)?,
                    })
                },
            )
            .map_err(db)
    }
    pub fn export_settings(&self) -> Result<String> {
        serde_json::to_string_pretty(&self.preferences()?).map_err(|_| {
            Error::new(
                "ExportError",
                "settings-export",
                "Could not serialize settings",
            )
        })
    }
    pub fn add_measurement(&self, kind: &str, value: &serde_json::Value) -> Result<()> {
        if let Some(raw) = value.get("error").filter(|v| !v.is_null())
            && let Ok(error) = serde_json::from_value::<crate::Error>(raw.clone())
        {
            if self
                .log(
                    "ERROR",
                    "Diagnostics",
                    &error.stage,
                    &error.code,
                    &error.message,
                )
                .is_err()
            {
                let _ = self.log(
                    "ERROR",
                    "Diagnostics",
                    "measurement",
                    "Failed",
                    "Measurement failed; unsafe diagnostic detail omitted",
                );
            }
        } else {
            let _ = self.log(
                "INFO",
                "Diagnostics",
                "measurement",
                "Completed",
                "Diagnostic measurement completed",
            );
        }

        if !["latency", "speed"].contains(&kind) {
            return Err(Error::new(
                "MeasurementKind",
                "diagnostics-save",
                "Invalid diagnostic type",
            ));
        }
        let raw = zeroize::Zeroizing::new(serde_json::to_vec(value).map_err(|_| {
            Error::new(
                "MeasurementInvalid",
                "diagnostics-save",
                "Invalid diagnostic result",
            )
        })?);
        if raw.len() > 1024 * 1024 {
            return Err(Error::new(
                "MeasurementLimit",
                "diagnostics-save",
                "Diagnostic result exceeds 1 MiB",
            ));
        }
        self.db
            .execute(
                "INSERT INTO measurements(kind,timestamp,sealed) VALUES(?1,?2,?3)",
                params![kind, now() as i64, vault::seal(&raw)?],
            )
            .map_err(db)?;
        self.db.execute("DELETE FROM measurements WHERE kind=?1 AND id NOT IN (SELECT id FROM measurements WHERE kind=?1 ORDER BY id DESC LIMIT 100)",[kind]).map_err(db)?;
        Ok(())
    }
    pub fn measurements(&self, kind: &str) -> Result<Vec<serde_json::Value>> {
        let mut stmt = self
            .db
            .prepare("SELECT sealed FROM measurements WHERE kind=?1 ORDER BY id DESC LIMIT 100")
            .map_err(db)?;
        let rows = stmt
            .query_map([kind], |r| r.get::<_, Vec<u8>>(0))
            .map_err(db)?;
        let mut values = vec![];
        for row in rows {
            let raw = vault::open(&row.map_err(db)?)?;
            let value: serde_json::Value = serde_json::from_slice(&raw).map_err(|_| {
                Error::new(
                    "MeasurementInvalid",
                    "diagnostics-read",
                    "Corrupt measurement",
                )
            })?;
            // Previous TCP results could measure our local TUN SYN response.
            // Keep the encrypted history, but require a new outside-TUN sample.
            if kind == "latency"
                && value["method"] == "tcp_connect_rtt"
                && !value["note"]
                    .as_str()
                    .is_some_and(|s| s.contains("outside NetPulse TUN"))
            {
                continue;
            }
            values.push(value);
        }
        Ok(values)
    }
    pub fn backup(&self) -> Result<Vec<u8>> {
        let ids = self.profiles()?;
        let profiles = ids
            .iter()
            .map(|p| self.profile(&p.id))
            .collect::<Result<Vec<_>>>()?;
        let subscriptions = self
            .subscriptions()?
            .into_iter()
            .map(|s| {
                let url = self.subscription_url(&s.id)?;
                Ok(serde_json::json!({"summary":s,"url":url.as_str()}))
            })
            .collect::<Result<Vec<_>>>()?;
        let bytes=Zeroizing::new(serde_json::to_vec(&serde_json::json!({"version":2,"latency":self.measurements("latency")?,"speed":self.measurements("speed")?,"preferences":self.preferences()?,"profiles":profiles,"subscriptions":subscriptions})).map_err(|_|Error::new("BackupError","backup","Could not encode backup"))?);
        vault::seal(&bytes)
    }
    pub fn restore(&mut self, encrypted: &[u8]) -> Result<()> {
        if encrypted.len() > 64 * 1024 * 1024 {
            return Err(Error::new(
                "BackupLimit",
                "restore",
                "Backup exceeds 64 MiB",
            ));
        }
        let raw = vault::open(encrypted)?;
        let value: serde_json::Value = serde_json::from_slice(&raw)
            .map_err(|_| Error::new("BackupInvalid", "restore", "Backup document is invalid"))?;
        if value["version"] != 1 && value["version"] != 2 {
            return Err(Error::new(
                "BackupVersion",
                "restore",
                "Unsupported backup version",
            ));
        }
        let prefs: Preferences = serde_json::from_value(value["preferences"].clone())
            .map_err(|_| Error::new("BackupInvalid", "restore", "Backup settings are invalid"))?;
        prefs.validate()?;
        let profiles: Vec<Profile> = serde_json::from_value(value["profiles"].clone())
            .map_err(|_| Error::new("BackupInvalid", "restore", "Backup profiles are invalid"))?;
        if profiles.len() > 20000 {
            return Err(Error::new(
                "BackupLimit",
                "restore",
                "Backup has too many profiles",
            ));
        }
        let mut rows = vec![];
        for p in profiles {
            p.validate()?;
            let fp = Sha256::digest(serde_json::to_vec(&p.outbound).unwrap())
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>();
            let raw = Zeroizing::new(serde_json::to_vec(&p).unwrap());
            rows.push((
                p.id.clone(),
                fp,
                p.subscription_id.clone(),
                vault::seal(&raw)?,
            ));
        }
        let mut subs = vec![];
        for s in value["subscriptions"].as_array().ok_or_else(|| {
            Error::new(
                "BackupInvalid",
                "restore",
                "Backup subscriptions are invalid",
            )
        })? {
            let summary: SubscriptionSummary = serde_json::from_value(s["summary"].clone())
                .map_err(|_| {
                    Error::new("BackupInvalid", "restore", "Invalid subscription metadata")
                })?;
            let url = s["url"].as_str().ok_or_else(|| {
                Error::new("BackupInvalid", "restore", "Missing subscription URL")
            })?;
            let u = url::Url::parse(url)
                .map_err(|_| Error::new("BackupInvalid", "restore", "Invalid subscription URL"))?;
            if u.scheme() != "https"
                || u.host_str().is_none()
                || !u.username().is_empty()
                || u.password().is_some()
            {
                return Err(Error::new(
                    "BackupInvalid",
                    "restore",
                    "Subscription must use HTTPS without URL userinfo",
                ));
            }
            subs.push((summary, vault::seal(url.as_bytes())?));
        }
        let mut measurements = vec![];
        for kind in ["latency", "speed"] {
            if let Some(values) = value[kind].as_array() {
                if values.len() > 100 {
                    return Err(Error::new(
                        "BackupLimit",
                        "restore",
                        "Too many measurements",
                    ));
                }
                for item in values.iter().rev() {
                    let raw = Zeroizing::new(serde_json::to_vec(item).unwrap());
                    if raw.len() > 1024 * 1024 {
                        return Err(Error::new(
                            "BackupLimit",
                            "restore",
                            "Oversized diagnostic history",
                        ));
                    }
                    measurements.push((kind, vault::seal(&raw)?));
                }
            }
        }
        let tx = self.db.transaction().map_err(db)?;
        tx.execute_batch("DELETE FROM profiles;DELETE FROM subscriptions;DELETE FROM settings;DELETE FROM measurements;")
            .map_err(db)?;
        tx.execute(
            "INSERT INTO settings VALUES(1,?1)",
            [serde_json::to_string(&prefs).unwrap()],
        )
        .map_err(db)?;
        for (id, fp, sub, sealed) in rows {
            tx.execute(
                "INSERT INTO profiles VALUES(?1,?2,?3,?4)",
                params![id, fp, sub, sealed],
            )
            .map_err(db)?;
        }
        for (s, url) in subs {
            if s.config_locked {
                tx.execute(
                    "INSERT OR IGNORE INTO subscription_locks VALUES(?1)",
                    [&s.id],
                )
                .map_err(db)?;
            }
            tx.execute(
                "INSERT INTO subscriptions(id,name,sealed_url,updated_at,interval_hours,url_protected,usage) VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![s.id, s.name, url, s.updated_at as i64, s.interval_hours, s.url_protected, s.usage.as_ref().map(|v| serde_json::to_string(v).unwrap())],
            )
            .map_err(db)?;
        }
        for (kind, sealed) in measurements {
            tx.execute(
                "INSERT INTO measurements(kind,timestamp,sealed) VALUES(?1,?2,?3)",
                params![kind, now() as i64, sealed],
            )
            .map_err(db)?;
        }
        tx.commit().map_err(db)
    }
    pub fn clear_all(&mut self) -> Result<()> {
        match std::fs::remove_file(self.directory.join("applications.json")) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(Error::io("applications-clear", e)),
        }
        self.db.execute_batch("BEGIN IMMEDIATE;DELETE FROM profiles;DELETE FROM subscriptions;DELETE FROM settings;DELETE FROM measurements;DELETE FROM events;COMMIT;PRAGMA wal_checkpoint(TRUNCATE);VACUUM;").map_err(db)
    }
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn provider_usage_parses_spacing_and_rejects_untrusted_numbers() {
        let a =
            SubscriptionUsage::parse("upload=1;download=2; total=10;expire=1799999999").unwrap();
        assert_eq!(a.upload, Some(1));
        assert_eq!(a.download, Some(2));
        assert_eq!(a.total, Some(10));
        assert!(SubscriptionUsage::parse("total=10;total=20").is_none());
        assert!(SubscriptionUsage::parse("total=-1").is_none());
        assert!(SubscriptionUsage::parse("upload=18446744073709551616").is_none());
        assert!(SubscriptionUsage::parse("").is_none());
    }
    #[test]
    fn json_edits_survive_refresh_and_lock_survives_older_restore() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Store::open(&dir.path().join("state.db")).unwrap();
        let sub = s
            .add_subscription("fixture", "https://example.org/private-fixture")
            .unwrap();
        let source = "trojan://fixture@example.org:443#Fixture";
        s.import(crate::imports::parse(source).unwrap(), Some(&sub))
            .unwrap();
        let id = s.profiles().unwrap()[0].id.clone();
        let before_lock = s.backup().unwrap();
        let mut p = s.profile(&id).unwrap();
        let mut outbound = p.outbound.clone();
        outbound["server"] = serde_json::json!("edited.example.org");
        p.outbound_override = Some(outbound);
        s.save_profile_transport(&p).unwrap();
        let mut parsed = crate::imports::parse(source).unwrap();
        parsed.usage =
            SubscriptionUsage::parse("upload=10;download=20;total=100;expire=1799999999");
        s.import(parsed, Some(&sub)).unwrap();
        assert_eq!(
            s.profile(&id).unwrap().resolved().unwrap().outbound["server"],
            "edited.example.org"
        );
        assert_eq!(
            s.subscriptions().unwrap()[0].usage.as_ref().unwrap().total,
            Some(100)
        );
        let with_usage = s.backup().unwrap();
        s.restore(&with_usage).unwrap();
        assert_eq!(
            s.subscriptions().unwrap()[0]
                .usage
                .as_ref()
                .unwrap()
                .download,
            Some(20)
        );
        s.lock_subscription(&sub).unwrap();
        assert_eq!(
            s.save_profile_transport(&p).unwrap_err().code,
            "ConfigurationLocked"
        );
        s.restore(&before_lock).unwrap();
        assert!(s.subscriptions().unwrap()[0].config_locked);
        assert_eq!(
            s.ensure_profile_editable(&id).unwrap_err().code,
            "ConfigurationLocked"
        );
    }

    #[test]
    fn subscription_edit_preserves_profiles_encryption_and_refresh_timestamp() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = super::Store::open(&dir.path().join("state.db")).unwrap();
        let id = s
            .add_subscription("Old", "https://example.org/EDIT-FIXTURE")
            .unwrap();
        s.import(
            crate::imports::parse("socks5://127.0.0.1:1080#Fixture").unwrap(),
            Some(&id),
        )
        .unwrap();
        let profile_id = s.profiles().unwrap()[0].id.clone();
        let before = s.subscriptions().unwrap()[0].updated_at;
        let mut summary = s.subscriptions().unwrap()[0].clone();
        summary.updated_at = 1000;
        summary.interval_hours = 1;
        assert!(!summary.due(999));
        assert!(!summary.due(4599));
        assert!(summary.due(4600));
        s.edit_subscription(&id, "  New name  ", 24, false).unwrap();
        assert_eq!(s.subscriptions().unwrap()[0].name, "New name");
        assert_eq!(s.subscriptions().unwrap()[0].interval_hours, 24);
        assert_eq!(s.subscriptions().unwrap()[0].updated_at, before);
        assert_eq!(s.profiles().unwrap()[0].id, profile_id);
        assert_eq!(
            s.copyable_subscription_url(&id).unwrap().as_str(),
            "https://example.org/EDIT-FIXTURE"
        );
        for (name, hours) in [("", 12), ("bad\nname", 12), ("Name", 0), ("Name", 169)] {
            assert!(s.edit_subscription(&id, name, hours, true).is_err());
        }
        assert_eq!(s.subscriptions().unwrap()[0].name, "New name");
        s.edit_subscription(&id, "New name", 24, true).unwrap();
        assert!(s.copyable_subscription_url(&id).is_err());
        let backup = s.backup().unwrap();
        s.clear_all().unwrap();
        s.restore(&backup).unwrap();
        assert_eq!(s.subscriptions().unwrap()[0].name, "New name");
        assert_eq!(s.subscriptions().unwrap()[0].interval_hours, 24);
        assert!(s.copyable_subscription_url(&id).is_err());
        assert_eq!(s.profiles().unwrap()[0].id, profile_id);
    }
    #[test]
    fn profile_transport_override_survives_subscription_refresh_and_backup() {
        use super::*;
        use crate::transport_options::ProfileTransport;
        let d = tempfile::tempdir().unwrap();
        let mut store = Store::open(&d.path().join("state.db")).unwrap();
        let id = store
            .add_subscription("fixture", "https://example.org/subscription")
            .unwrap();
        let uri = "trojan://FIXTURE-SECRET@example.org:443#Fixture";
        store
            .import(crate::imports::parse(uri).unwrap(), Some(&id))
            .unwrap();
        let mut p = store.profile(&store.profiles().unwrap()[0].id).unwrap();
        let original = p.outbound.clone();
        p.transport_override = Some(ProfileTransport {
            sni_mode: "custom".into(),
            server_name: "local.example.org".into(),
            http_host: None,
            fingerprint: Some("chrome".into()),
        });
        store.save_profile_transport(&p).unwrap();
        store
            .import(crate::imports::parse(uri).unwrap(), Some(&id))
            .unwrap();
        assert_eq!(store.profiles().unwrap().len(), 1);
        assert_eq!(
            store.profile(&p.id).unwrap().transport_override,
            p.transport_override
        );
        let backup = store.backup().unwrap();
        store.clear_all().unwrap();
        store.restore(&backup).unwrap();
        let mut restored = store.profile(&p.id).unwrap();
        assert_eq!(
            restored.resolved().unwrap().outbound["tls"]["server_name"],
            "local.example.org"
        );
        restored.transport_override = None;
        store.save_profile_transport(&restored).unwrap();
        assert_eq!(store.profile(&p.id).unwrap().outbound, original);
        assert!(
            !serde_json::to_string(&store.profiles().unwrap())
                .unwrap()
                .contains("FIXTURE-SECRET")
        );
    }
    use super::*;
    #[test]
    fn persistence_dedup_and_no_plaintext() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.db");
        let mut s = Store::open(&path).unwrap();
        let uri = "trojan://SECRET-NEVER-STORE-PLAIN@example.org:443?fm=";
        assert_eq!(
            s.import(crate::imports::parse(uri).unwrap(), None)
                .unwrap()
                .imported,
            1
        );
        assert_eq!(
            s.import(crate::imports::parse(uri).unwrap(), None)
                .unwrap()
                .imported,
            0
        );
        let id = s.profiles().unwrap()[0].id.clone();
        assert_eq!(
            s.profile(&id).unwrap().outbound["password"],
            "SECRET-NEVER-STORE-PLAIN"
        );
        drop(s);
        let raw = std::fs::read(&path).unwrap();
        let secret = b"SECRET-NEVER-STORE-PLAIN";
        assert!(!raw.windows(secret.len()).any(|b| b == secret));
        assert_eq!(Store::open(&path).unwrap().profiles().unwrap().len(), 1);
    }
    #[test]
    fn migrations_encrypted_history_and_atomic_failed_restore() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.db");
        {
            let s = Store::open(&path).unwrap();
            s.db.execute_batch("PRAGMA user_version=1").unwrap();
        }
        let mut s = Store::open(&path).unwrap();
        assert_eq!(
            s.db.query_row("PRAGMA user_version", [], |r| r.get::<_, u32>(0))
                .unwrap(),
            4
        );
        assert!(
            std::fs::read_dir(dir.path())
                .unwrap()
                .flatten()
                .any(|f| f.path().extension().is_some_and(|e| e == "npdb"))
        );
        let prefs = Preferences {
            favorites: vec!["fixture".into()],
            selected_profile: Some("fixture".into()),
            ..Preferences::default()
        };
        s.save_preferences(&prefs).unwrap();
        s.add_measurement("latency",&serde_json::json!({"profile_id":"fixture","avg_ms":null,"destination":"PRIVATE-TEST-ENDPOINT"})).unwrap();
        let backup = s.backup().unwrap();
        let mut invalid: serde_json::Value =
            serde_json::from_slice(&vault::open(&backup).unwrap()).unwrap();
        invalid["profiles"] = serde_json::json!([{"name":"invalid"}]);
        let invalid = vault::seal(&serde_json::to_vec(&invalid).unwrap()).unwrap();
        assert!(s.restore(&invalid).is_err());
        assert_eq!(s.preferences().unwrap().favorites, vec!["fixture"]);
        assert_eq!(s.measurements("latency").unwrap().len(), 1);
        s.clear_all().unwrap();
        s.restore(&backup).unwrap();
        assert_eq!(s.measurements("latency").unwrap().len(), 1);
        drop(s);
        let bytes = std::fs::read(&path).unwrap();
        assert!(!bytes.windows(21).any(|w| w == b"PRIVATE-TEST-ENDPOINT"));
        {
            let db = Connection::open(&path).unwrap();
            db.execute_batch("PRAGMA user_version=999").unwrap();
        }
        assert_eq!(Store::open(&path).err().unwrap().code, "SchemaTooNew");
    }
    #[test]
    fn unsafe_log_blocked() {
        let d = tempfile::tempdir().unwrap();
        let s = Store::open(&d.path().join("state.db")).unwrap();
        assert!(
            s.log(
                "ERROR",
                "Profiles",
                "parse",
                "ImportError",
                "vless://PRIVATE"
            )
            .is_err()
        );
        assert!(s.logs().unwrap().is_empty());
    }
    #[test]
    fn subscription_copy_policy_migrates_and_survives_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.db");
        let mut s = Store::open(&path).unwrap();
        let protected = s
            .add_subscription("Protected", "https://example.org/SECRET-FIXTURE")
            .unwrap();
        let copyable = s
            .add_subscription_with_protection("Copyable", "https://example.org/COPY-FIXTURE", false)
            .unwrap();
        assert_eq!(
            s.copyable_subscription_url(&protected).unwrap_err().code,
            "SubscriptionProtected"
        );
        assert_eq!(
            s.copyable_subscription_url(&copyable).unwrap().as_str(),
            "https://example.org/COPY-FIXTURE"
        );
        let backup = s.backup().unwrap();
        s.clear_all().unwrap();
        s.restore(&backup).unwrap();
        assert!(s.copyable_subscription_url(&protected).is_err());
        assert!(s.copyable_subscription_url(&copyable).is_ok());
        s.db.execute_batch(
            "ALTER TABLE subscriptions DROP COLUMN url_protected; PRAGMA user_version=2;",
        )
        .unwrap();
        drop(s);
        let s = Store::open(&path).unwrap();
        assert!(s.subscriptions().unwrap().iter().all(|s| s.url_protected));
        assert!(s.copyable_subscription_url(&copyable).is_err());
        drop(s);
        let raw = std::fs::read(path).unwrap();
        assert!(
            !raw.windows(b"COPY-FIXTURE".len())
                .any(|w| w == b"COPY-FIXTURE")
        );
    }
    #[test]
    fn server_summary_uses_https_measurements_only() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(&dir.path().join("state.db")).unwrap();
        store
            .save_preferences(&Preferences {
                ping_method: "proxy_https_rtt".into(),
                ..Preferences::default()
            })
            .unwrap();
        store
            .import(
                crate::imports::parse("socks5://127.0.0.1:1080#Fixture").unwrap(),
                None,
            )
            .unwrap();
        let id = store.profiles().unwrap()[0].id.clone();
        store.add_measurement("latency",&serde_json::json!({"profile_id":id,"method":"tcp_connect_rtt","avg_ms":1.0,"timestamp":1})).unwrap();
        assert!(store.profiles().unwrap()[0].latency_ms.is_none());
        store.add_measurement("latency",&serde_json::json!({"profile_id":id,"method":"proxy_https_rtt","avg_ms":145.0,"timestamp":2})).unwrap();
        store.add_measurement("latency",&serde_json::json!({"profile_id":id,"method":"tcp_connect_rtt","avg_ms":2.0,"timestamp":3})).unwrap();
        assert_eq!(store.profiles().unwrap()[0].latency_ms, Some(145));
    }
}
