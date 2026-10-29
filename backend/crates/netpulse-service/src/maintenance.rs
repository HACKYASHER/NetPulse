//! File transaction used after SCM confirms that our service has stopped.
//! Unchanged components are never copied. External removal stops recovery;
//! the updater must not recreate a file quarantined by security software.
use netpulse_core::{Error, Result, core_releases::hash};
use std::path::{Path, PathBuf};

pub const FILES: [&str; 6] = ["NetPulseService.exe", "sing-box.exe", "libcronet.dll", "xray.exe", "sing-box-release.json", "xray-release.json"];
/// An absent SCM registration may leave our protected transaction directories
/// after quarantine or rollback. Preserve these originals for investigation;
/// they are never loaded as part of a fresh installation.
pub fn validate_fresh_install_directory(directory: &Path) -> Result<()> {
    use std::os::windows::fs::MetadataExt;
    let foreign=||Error::new("ExistingInstallFiles","service-install","В папке службы остались действующие или посторонние файлы. Установка остановлена; содержимое сохранено.");
    for entry in std::fs::read_dir(directory).map_err(|e|Error::io("service-directory",e))? {
        let entry=entry.map_err(|e|Error::io("service-directory",e))?;
        let name=entry.file_name();let name=name.to_string_lossy();
        let metadata=std::fs::symlink_metadata(entry.path()).map_err(|_|foreign())?;
        if !name.strip_prefix("upgrade-").is_some_and(|id|uuid::Uuid::parse_str(id).is_ok())
            || !metadata.is_dir() || metadata.file_attributes()&0x400!=0 {return Err(foreign());}
        for file in std::fs::read_dir(entry.path()).map_err(|_|foreign())? {
            let file=file.map_err(|_|foreign())?;let name=file.file_name();let name=name.to_string_lossy();
            let expected=name.strip_prefix("previous-").unwrap_or(&name);
            let metadata=std::fs::symlink_metadata(file.path()).map_err(|_|foreign())?;
            if !FILES.contains(&expected)||!metadata.is_file()||metadata.file_attributes()&0x400!=0 {return Err(foreign());}
        }
    }
    Ok(())
}
struct Change {
    name: &'static str,
    before: Option<String>,
    after: Option<String>,
    backed_up: bool,
    installed: bool,
}
pub struct Upgrade {
    directory: PathBuf,
    stage: PathBuf,
    changes: Vec<Change>,
    preserve: bool,
}
fn interrupted() -> Error {
    Error::new("ServiceUpdateInterrupted", "service-upgrade", "Обновление службы остановлено: файлы исчезли, изменились или заблокированы. Проверьте отчёт антивируса. Автоматическое восстановление заблокированных файлов не выполняется.")
}
fn digest(path: &Path) -> Result<Option<String>> {
    use std::os::windows::fs::MetadataExt;
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && metadata.file_attributes() & 0x400 == 0 => hash(path).map(Some).map_err(|_|interrupted()),
        Ok(_) => Err(interrupted()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(interrupted()),
    }
}
impl Upgrade {
    pub fn prepare(directory: &Path, sources: &[(&'static str, Option<PathBuf>)], approvals: &[netpulse_core::core_releases::Approval], protect: impl Fn(&Path) -> Result<()>) -> Result<Self> {
        let stage = directory.join(format!("upgrade-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&stage).map_err(|e| Error::io("service-upgrade-stage", e))?;
        let mut transaction = Self { directory: directory.to_owned(), stage, changes: Vec::new(), preserve: false };
        protect(&transaction.stage)?;
        // Serialize the authenticated records, rather than rereading mutable
        // approval files from the unelevated application's source directory.
        netpulse_core::core_releases::write_approvals(&transaction.stage, approvals)?;
        let mut files = sources.to_vec();
        for (name, engine) in [(FILES[4], "sing-box"), (FILES[5], "xray")] {
            files.push((name, approvals.iter().any(|record|record.release.engine==engine).then(||transaction.stage.join(name))));
        }
        for (name, source) in &files {
            if !FILES.contains(name) { return Err(interrupted()); }
            let before = digest(&directory.join(name))?;
            let after = match source {
                Some(source) => Some(digest(source)?.ok_or_else(interrupted)?),
                None => None,
            };
            if before == after { continue; }
            if let Some(source) = source {
                let staged = transaction.stage.join(name);
                if source != &staged { std::fs::copy(source, &staged).map_err(|_| interrupted())?; }
                protect(&staged)?;
                if digest(&staged)? != after { return Err(interrupted()); }
            }
            transaction.changes.push(Change { name, before, after, backed_up: false, installed: false });
        }
        Ok(transaction)
    }
    pub fn apply(&mut self, protect: impl Fn(&Path) -> Result<()>, verify: impl FnOnce() -> Result<()>) -> Result<()> {
        // Recheck the complete transaction before changing the first live file.
        for change in &self.changes {
            if digest(&self.directory.join(change.name))? != change.before
                || digest(&self.stage.join(change.name))? != change.after { return Err(interrupted()); }
        }
        // Retain recoverable originals if an unexpected exit interrupts commit.
        self.preserve = true;
        let result = (|| {
            for change in &mut self.changes {
                let target = self.directory.join(change.name);
                if change.before.is_some() {
                    std::fs::rename(&target, self.stage.join(format!("previous-{}", change.name)))
                        .map_err(|e| Error::io("service-upgrade-backup", e))?;
                    change.backed_up = true;
                }
                if change.after.is_some() {
                    std::fs::rename(self.stage.join(change.name), &target)
                        .map_err(|e| Error::io("service-upgrade-commit", e))?;
                    change.installed = true;
                    protect(&target)?;
                }
            }
            for change in &self.changes {
                if digest(&self.directory.join(change.name))? != change.after { return Err(interrupted()); }
            }
            verify()
        })();
        if let Err(error) = result {
            if matches!(error.code.as_str(), "ServiceComponentsMissing" | "ServiceComponentsUnavailable" | "ServiceUpdateInterrupted" | "RuntimeComponentUnavailable" | "EngineHashMismatch" | "CoreApprovalMismatch") {
                return Err(interrupted());
            }
            // Validate every artifact before rollback. An externally deleted or
            // changed candidate is preserved for investigation, never restored.
            for change in &self.changes {
                let current = digest(&self.directory.join(change.name))?;
                let expected = if change.installed { &change.after } else if change.backed_up { &None } else { &change.before };
                if &current != expected
                    || (!change.installed && digest(&self.stage.join(change.name))? != change.after)
                    || (change.backed_up && digest(&self.stage.join(format!("previous-{}",change.name)))? != change.before) {
                    return Err(interrupted());
                }
            }
            for change in self.changes.iter().rev() {
                let target = self.directory.join(change.name);
                if change.installed { std::fs::remove_file(&target).map_err(|_| interrupted())?; }
                if change.backed_up {
                    std::fs::rename(self.stage.join(format!("previous-{}",change.name)), &target).map_err(|_| interrupted())?;
                }
            }
            self.preserve = false;
            return Err(error);
        }
        self.preserve = false;
        Ok(())
    }
}
impl Drop for Upgrade {
    fn drop(&mut self) {
        if self.preserve { return; }
        for name in FILES {
            for file in [self.stage.join(name), self.stage.join(format!("previous-{name}"))] {
                // Never follow a replaced link while cleaning our own staging directory.
                if digest(&file).ok().flatten().is_some() { let _ = std::fs::remove_file(file); }
            }
        }
        let _ = std::fs::remove_dir(&self.stage);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self { let path=std::env::temp_dir().join(format!("netpulse-maintenance-{}",uuid::Uuid::new_v4()));std::fs::create_dir(&path).unwrap();Self(path) }
        fn file(&self,name:&str,bytes:&[u8])->PathBuf { let path=self.0.join(name);std::fs::write(&path,bytes).unwrap();path }
    }
    impl Drop for Fixture { fn drop(&mut self) {
        let base=std::env::temp_dir().canonicalize().unwrap();
        let target=self.0.canonicalize().unwrap();
        assert_eq!(target.parent(),Some(base.as_path()));
        assert!(target.file_name().unwrap().to_string_lossy().starts_with("netpulse-maintenance-"));
        let _=std::fs::remove_dir_all(target);
    } }
    #[test]
    fn explicit_repair_can_populate_an_absent_bundle_and_preserves_unknown_files() {
        let f=Fixture::new();let source=Fixture::new();let unknown=f.file("user-note.txt",b"keep");
        let files=["NetPulseService.exe","sing-box.exe","libcronet.dll","xray.exe"];
        let sources=files.iter().map(|name|(*name,Some(source.file(name,name.as_bytes())))).collect::<Vec<_>>();
        let mut tx=Upgrade::prepare(&f.0,&sources,&[],|_|Ok(())).unwrap();
        tx.apply(|_|Ok(()),||netpulse_core::service::validate_service_files(&f.0)).unwrap();drop(tx);
        for name in files {assert_eq!(std::fs::read(f.0.join(name)).unwrap(),name.as_bytes());}
        assert_eq!(std::fs::read(unknown).unwrap(),b"keep");
    }
    #[test]
    fn removal_during_explicit_repair_is_not_retried_or_rolled_back_over_detection() {
        let f=Fixture::new();let source=Fixture::new();let candidate=source.file(FILES[0],b"verified fixture");
        let mut tx=Upgrade::prepare(&f.0,&[(FILES[0],Some(candidate))],&[],|_|Ok(())).unwrap();
        let target=f.0.join(FILES[0]);
        assert_eq!(tx.apply(|_|Ok(()),||{std::fs::remove_file(&target).unwrap();Err(Error::new("ServiceComponentsMissing","verify","removed"))}).unwrap_err().code,"ServiceUpdateInterrupted");
        assert!(!target.exists());
    }
    #[test]
    fn orphaned_owned_updates_do_not_block_fresh_install_and_are_preserved() {
        let f=Fixture::new();validate_fresh_install_directory(&f.0).unwrap();
        let stage=f.0.join(format!("upgrade-{}",uuid::Uuid::new_v4()));std::fs::create_dir(&stage).unwrap();
        let original=stage.join("previous-NetPulseService.exe");std::fs::write(&original,b"preserve original").unwrap();
        validate_fresh_install_directory(&f.0).unwrap();assert_eq!(std::fs::read(&original).unwrap(),b"preserve original");
        let foreign=stage.join("unrelated.txt");std::fs::write(&foreign,b"preserve unknown").unwrap();
        assert!(validate_fresh_install_directory(&f.0).is_err());std::fs::remove_file(&foreign).unwrap();
        f.file("NetPulseService.exe",b"existing live component");assert!(validate_fresh_install_directory(&f.0).is_err());
        assert_eq!(std::fs::read(original).unwrap(),b"preserve original");
    }
    #[test]
    fn unchanged_cores_are_not_staged_and_success_cleans_backup() {
        let f=Fixture::new();let source=f.file("candidate",b"new service");f.file("NetPulseService.exe",b"old service");let core=f.file("sing-box.exe",b"same core");
        let stage;
        { let mut tx=Upgrade::prepare(&f.0,&[(FILES[0],Some(source)),(FILES[1],Some(core))], &[], |_|Ok(())).unwrap();stage=tx.stage.clone();assert!(!stage.join("sing-box.exe").exists());assert_eq!(tx.changes.len(),1);tx.apply(|_|Ok(()),||Ok(())).unwrap(); }
        assert!(!stage.exists());assert_eq!(std::fs::read(f.0.join(FILES[0])).unwrap(),b"new service");
    }
    #[test]
    fn rejected_candidate_restores_originals_and_cleans_stage() {
        let f=Fixture::new();let source=f.file("candidate",b"new");f.file(FILES[0],b"old");
        let stage;
        {let mut tx=Upgrade::prepare(&f.0,&[(FILES[0],Some(source))],&[],|_|Ok(())).unwrap();stage=tx.stage.clone();assert!(tx.apply(|_|Ok(()),||Err(Error::new("fixture","verify","reject"))).is_err());}
        assert_eq!(std::fs::read(f.0.join(FILES[0])).unwrap(),b"old");assert!(!stage.exists());
    }
    #[test]
    fn external_removal_does_not_recreate_quarantined_files() {
        let f=Fixture::new();let source=f.file("candidate",b"new");f.file(FILES[0],b"old");
        let stage;
        {let mut tx=Upgrade::prepare(&f.0,&[(FILES[0],Some(source))],&[],|_|Ok(())).unwrap();stage=tx.stage.clone();let target=f.0.join(FILES[0]);let error=tx.apply(|_|Ok(()),||{std::fs::remove_file(&target).unwrap();Err(interrupted())}).unwrap_err();assert_eq!(error.code,"ServiceUpdateInterrupted");}
        assert!(!f.0.join(FILES[0]).exists());assert_eq!(std::fs::read(stage.join("previous-NetPulseService.exe")).unwrap(),b"old");
    }
    #[test]
    fn disappearing_staged_file_preserves_all_live_files() {
        let f=Fixture::new();let source=f.file("candidate",b"new");f.file(FILES[0],b"old");
        let mut tx=Upgrade::prepare(&f.0,&[(FILES[0],Some(source))],&[],|_|Ok(())).unwrap();std::fs::remove_file(tx.stage.join(FILES[0])).unwrap();assert!(tx.apply(|_|Ok(()),||Ok(())).is_err());assert_eq!(std::fs::read(f.0.join(FILES[0])).unwrap(),b"old");
    }
    #[test]
    fn external_changes_abort_before_touching_live_bundle() {
        let f=Fixture::new();let source=f.file("candidate",b"new");f.file(FILES[0],b"old");
        let mut tx=Upgrade::prepare(&f.0,&[(FILES[0],Some(source))],&[],|_|Ok(())).unwrap();
        f.file(FILES[0],b"external change");assert!(tx.apply(|_|Ok(()),||Ok(())).is_err());
        assert_eq!(std::fs::read(f.0.join(FILES[0])).unwrap(),b"external change");
    }
    #[test]
    fn changed_candidate_stops_rollback_and_preserves_unknown_staging_files() {
        let f=Fixture::new();let source=f.file("candidate",b"new");f.file(FILES[0],b"old");
        let stage;
        {let mut tx=Upgrade::prepare(&f.0,&[(FILES[0],Some(source))],&[],|_|Ok(())).unwrap();stage=tx.stage.clone();
        std::fs::write(stage.join("unrelated.txt"),b"keep").unwrap();
        assert_eq!(tx.apply(|_|Ok(()),||{f.file(FILES[0],b"external");Err(interrupted())}).unwrap_err().code,"ServiceUpdateInterrupted");}
        assert_eq!(std::fs::read(f.0.join(FILES[0])).unwrap(),b"external");
        assert_eq!(std::fs::read(stage.join("unrelated.txt")).unwrap(),b"keep");
        assert_eq!(std::fs::read(stage.join("previous-NetPulseService.exe")).unwrap(),b"old");
    }
    #[test]
    fn removal_of_unchanged_core_also_stops_bundle_recovery() {
        let f=Fixture::new();let source=f.file("candidate",b"new");f.file(FILES[0],b"old");let core=f.file(FILES[1],b"unchanged");
        let mut tx=Upgrade::prepare(&f.0,&[(FILES[0],Some(source)),(FILES[1],Some(core.clone()))],&[],|_|Ok(())).unwrap();
        assert_eq!(tx.apply(|_|Ok(()),||{std::fs::remove_file(core).unwrap();Err(Error::new("ServiceComponentsUnavailable","verify","removed"))}).unwrap_err().code,"ServiceUpdateInterrupted");
        assert!(!f.0.join(FILES[1]).exists());
        assert_eq!(std::fs::read(f.0.join(FILES[0])).unwrap(),b"new");
        assert!(tx.stage.join("previous-NetPulseService.exe").exists());
    }
}
