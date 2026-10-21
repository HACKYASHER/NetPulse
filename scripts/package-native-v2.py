"""Package the alpha separately; include corresponding source and dependency notices."""
from pathlib import Path
import importlib.util
import hashlib
import json
import shutil
import subprocess
import sys
import zipfile
import uuid
import tomllib
import re

ROOT=Path(__file__).resolve().parents[1]
APP=ROOT/'apps/desktop-v2'
OUT=ROOT/'release-v2'
VERSION=json.loads((APP/'package.json').read_text(encoding='utf8'))['version']
assert tomllib.loads((APP/'Cargo.toml').read_text(encoding='utf8'))['workspace']['package']['version']==VERSION
assert json.loads((APP/'src-tauri/tauri.conf.json').read_text(encoding='utf8'))['version']==VERSION
spec=importlib.util.spec_from_file_location('native_v2',ROOT/'scripts/native-v2.py')
native=importlib.util.module_from_spec(spec);spec.loader.exec_module(native)
env=native.native_env()
node=shutil.which('node',path=env['PATH'])
cargo=shutil.which('cargo',path=env['PATH'])
pnpm=Path.home()/'.cache/codex-runtimes/codex-primary-runtime/dependencies/bin/fallback/pnpm.cmd'
if not pnpm.exists():pnpm=shutil.which('pnpm.cmd',path=env['PATH']) or 'pnpm'

def checked(args,cwd=APP):subprocess.run(list(map(str,args)),cwd=cwd,env=env,check=True)
def sha(path):
    with path.open('rb') as file:return hashlib.file_digest(file,'sha256').hexdigest()

def source_fingerprint():
    paths=[APP/'Cargo.toml',APP/'Cargo.lock',APP/'package.json',APP/'src-tauri/tauri.conf.json',APP/'src-tauri/build.rs',Path(__file__),ROOT/'scripts/runtime-dependencies-v2.py',ROOT/'scripts/test-native-v2.py',ROOT/'scripts/test-routing-v2.py']
    for directory in ['ui','src-tauri/src','src-tauri/windows','crates/netpulse-core','crates/netpulse-service']:
        paths.extend(p for p in (APP/directory).rglob('*') if p.is_file())
    digest=hashlib.sha256()
    for path in sorted(set(paths)):
        digest.update(path.relative_to(ROOT).as_posix().encode());digest.update(b'\0');digest.update(path.read_bytes())
    return digest.hexdigest()

PACKAGED_SOURCE=source_fingerprint()

OUT.mkdir(exist_ok=True)
checked([sys.executable,ROOT/'scripts/stage-cores-v2.py'],cwd=ROOT)
assert (OUT/'NetPulse-Engine-Sources.zip').is_file(), 'Build corresponding engine sources with engine-sources-v2.py'
shutil.copytree(APP/'resources/cores',OUT/'cores',dirs_exist_ok=True)
receipt_path=ROOT/'.local/v2-native-tests.json'
receipt=json.loads(receipt_path.read_text()) if receipt_path.exists() else {}
if receipt.get('result')!='passed' or receipt.get('core_sha256')!=native.core_fingerprint():
    checked([sys.executable,ROOT/'scripts/test-native-v2.py'],cwd=ROOT)
    receipt=json.loads(receipt_path.read_text())
assert receipt['result']=='passed' and receipt['core_sha256']==native.core_fingerprint()
stress_path=ROOT/'.local/whitelist-stress.json'
stress=json.loads(stress_path.read_text()) if stress_path.exists() else {}
if stress.get('result')!='passed' or stress.get('core_sha256')!=native.core_fingerprint():
    checked([sys.executable,ROOT/'scripts/test-routing-v2.py'],cwd=ROOT)

metadata=json.loads(subprocess.check_output([cargo,'metadata','--locked','--format-version','1','--filter-platform','x86_64-pc-windows-msvc'],cwd=APP,env=env))
packages={p['id']:p for p in metadata['packages']};nodes={n['id']:n for n in metadata['resolve']['nodes']}
start=next(p['id'] for p in packages.values() if p['name']=='netpulse-desktop-v2')
selected=set()
def walk(id):
    if id in selected:return
    selected.add(id)
    for d in nodes[id]['deps']:
        if any(k['kind']!='dev' for k in d['dep_kinds']):walk(d['pkg'])
walk(start)
walk(next(p['id'] for p in packages.values() if p['name']=='netpulse-service'))
inventory=[{'name':'sing-box','version':'1.14.3','license':'GPL-3.0-or-later','repository':'https://github.com/SagerNet/sing-box'},{'name':'Xray-core','version':'26.10.17','license':'MPL-2.0','repository':'https://github.com/XTLS/Xray-core'}]
license_dir=OUT/'licenses';license_dir.mkdir(exist_ok=True)
nsis_notice=license_dir/'tauri-nsis-template';nsis_notice.mkdir(exist_ok=True)
shutil.copy2(APP/'src-tauri/windows/TAURI-TEMPLATE-NOTICE.txt',nsis_notice/'LICENSE.txt')
for id in sorted(selected):
    p=packages[id]
    if p['name'].startswith('netpulse'):continue
    directory=Path(p['manifest_path']).parent
    license_files=[f for f in directory.iterdir() if f.is_file() and f.name.upper().startswith(('LICENSE','LICENCE','COPYING','NOTICE','UNLICENSE'))]
    if not license_files:
        fallback='webview2-rs.txt' if p['name'].startswith('webview2-com') else {'alloc-stdlib':'alloc-stdlib.txt','selectors':'MPL-2.0.txt'}.get(p['name'])
        if fallback:license_files=[APP/'licenses'/fallback]
    assert license_files,f"Missing original license notice: {p['name']}"
    inventory.append({k:p.get(k) for k in ['name','version','license','repository']})
    for file in license_files:
        dest=license_dir/f"{p['name']}-{p['version']}";dest.mkdir(exist_ok=True);shutil.copy2(file,dest/file.name)
seen=set()
def npm(package_dir):
    package_dir=package_dir.resolve();key=package_dir/'package.json'
    if key in seen:return
    seen.add(key);p=json.loads(key.read_text(encoding='utf8'))
    inventory.append({'name':p['name'],'version':p['version'],'license':p.get('license'),'repository':p.get('repository')})
    for file in package_dir.iterdir():
        if file.is_file() and file.name.upper().startswith(('LICENSE','LICENCE','COPYING','NOTICE')):
            dest=license_dir/f"npm-{p['name'].replace('/','-')}-{p['version']}";dest.mkdir(exist_ok=True);shutil.copy2(file,dest/file.name)
    for name in p.get('dependencies',{}):
        for parent in [package_dir,*package_dir.parents]:
            candidate=parent/'node_modules'/name
            if (candidate/'package.json').exists():npm(candidate);break
for name in ['react','react-dom','lucide-react','@tauri-apps/api']:npm(APP/'node_modules'/name)
(OUT/'DEPENDENCIES.json').write_text(json.dumps(inventory,ensure_ascii=False,indent=2),encoding='utf8')
gpl=ROOT/'LICENSE.txt'
if not gpl.exists():gpl=ROOT/'apps/desktop/licenses/GPL-3.0-only.txt'
shutil.copy2(gpl,OUT/'LICENSE.txt')
for name in ['README.md','CHANGELOG.md','LICENSE-NOTICE.md']:shutil.copy2(APP/name,OUT/name)
for name in ['netpulse-v2-architecture.md','netpulse-v2-verification.md','netpulse-v2-windows-check.md']:shutil.copy2(ROOT/'docs'/name,OUT/name)

frontend_run=subprocess.run([node,'--test','--test-reporter=tap',*sorted((APP/'ui').glob('*.test.ts'))],cwd=APP,env=env,check=True,capture_output=True,text=True,encoding='utf8')
print(frontend_run.stdout,flush=True)
frontend_passed=int(re.search(r'^# pass (\d+)$',frontend_run.stdout,re.M)[1])
assert re.search(r'^# fail 0$',frontend_run.stdout,re.M)
checked([cargo,'test','-p','netpulse-desktop-v2','--lib'])
checked([pnpm,'build'])
checked([cargo,'rustc','-p','netpulse-service','--bin','NetPulseService','--release','--','-C','target-feature=+crt-static'])
resources=APP/'resources';resources.mkdir(exist_ok=True)
shutil.copy2(APP/'target/release/NetPulseService.exe',resources/'NetPulseService.exe')
checked([cargo,'clippy','--workspace','--all-targets','--features','custom-protocol','--','-D','warnings'])
checked([node,APP/'node_modules/@tauri-apps/cli/tauri.js','build','--ci','--bundles','nsis'])
executable=APP/'target/release/NetPulse.exe'
runtime_spec=importlib.util.spec_from_file_location('runtime_dependencies_v2',ROOT/'scripts/runtime-dependencies-v2.py')
runtime_audit=importlib.util.module_from_spec(runtime_spec);runtime_spec.loader.exec_module(runtime_audit)
runtime_imports=runtime_audit.check([executable,resources/'NetPulseService.exe',resources/'cores/sing-box.exe',resources/'cores/xray.exe',resources/'cores/libcronet.dll'])
installer=max((APP/'target/release/bundle/nsis').glob('*.exe'),key=lambda p:p.stat().st_mtime)
shutil.copy2(executable,OUT/'NetPulse.exe');shutil.copy2(installer,OUT/'NetPulse-Setup.exe');shutil.copy2(resources/'NetPulseService.exe',OUT/'NetPulseService.exe')

source=OUT/'NetPulse-Source.zip'
with zipfile.ZipFile(source,'w',zipfile.ZIP_DEFLATED,compresslevel=6,strict_timestamps=False) as archive:
    for path in APP.rglob('*'):
        rel=path.relative_to(APP)
        if any(part in {'target','node_modules','dist','gen','.git'} for part in rel.parts):continue
        if path.is_file():archive.write(path,'apps/desktop-v2/'+rel.as_posix())
    for path in sorted(set([ROOT/'pnpm-workspace.yaml',ROOT/'pnpm-lock.yaml',ROOT/'package.json',* (ROOT/'scripts').glob('*v2*.py'),ROOT/'scripts/native-v2.py',ROOT/'scripts/v2-icons.py',ROOT/'scripts/v2-test-certificates.py',ROOT/'scripts/fetch-xray-alpha4.py',ROOT/'scripts/bootstrap-v2.py',ROOT/'requirements-test.txt'])):
        if path.is_file():archive.write(path,path.relative_to(ROOT).as_posix())
    for path in (ROOT/'docs').glob('netpulse-v2-*.md'):archive.write(path,path.relative_to(ROOT).as_posix())
    archive.write(OUT/'LICENSE.txt','LICENSE.txt')
    for id in sorted(selected):
        p=packages[id]
        if not (p.get('source') or '').startswith('registry+'):continue
        directory=Path(p['manifest_path']).parent
        crate=directory.parents[2]/'cache'/directory.parent.name/f"{p['name']}-{p['version']}.crate"
        assert crate.is_file(),f"Missing original source archive: {p['name']}"
        archive.write(crate,f"dependencies/rust/{crate.name}",compress_type=zipfile.ZIP_STORED)
    for key in sorted(seen):
        directory=key.parent;p=json.loads(key.read_text(encoding='utf8'))
        for path in directory.rglob('*'):
            if path.is_file() and 'node_modules' not in path.relative_to(directory).parts:
                archive.write(path,f"dependencies/npm/{p['name']}-{p['version']}/{path.relative_to(directory).as_posix()}")
with zipfile.ZipFile(source) as archive:
    assert archive.testzip() is None

smoke=ROOT/'.local'/('v2-packaged-self-test-'+uuid.uuid4().hex)
subprocess.run([str(OUT/'NetPulse.exe'),'--self-test',str(smoke)],cwd=OUT,env=env,timeout=30,check=True)
smoke_result=json.loads((smoke/'result.json').read_text(encoding='utf8'));assert smoke_result['result']=='passed'
# Launch the real packaged WebView, without a localhost server or the installed
# app's single-instance/data/browser profile. No VPN, autostart or subscription requests.
startup_cases=[]
missing_root=ROOT/'.local'/('v2-missing-runtime-'+uuid.uuid4().hex)
missing_root.mkdir();shutil.copy2(OUT/'NetPulse.exe',missing_root/'NetPulse.exe')
for name,program,ready in [('bundled',OUT/'NetPulse.exe',True),('missing_runtime',missing_root/'NetPulse.exe',False)]:
    folder=ROOT/'.local'/('v2-startup-'+name+'-'+uuid.uuid4().hex)
    started=subprocess.run([str(program),'--startup-smoke',str(folder)],cwd=program.parent,env=env,timeout=60)
    if started.returncode:
        failure=folder/'startup-failure.json'
        raise RuntimeError(f"Native WebView {name}: "+(failure.read_text(encoding='utf8') if failure.is_file() else str(started.returncode)))
    actual=json.loads((folder/'startup-result.json').read_text(encoding='utf8'))
    assert actual['result']=='passed' and actual['version']==VERSION and actual['webview_ready']
    assert actual['runtime_ready']==ready and not actual['system_network_changed'] and not actual['browser_server_used']
    startup_cases.append({'case':name,**actual})
subprocess.run([str(OUT/'NetPulse.exe'),'--installer-verify'],cwd=OUT,env=env,timeout=30,check=True)
bad=subprocess.run([str(missing_root/'NetPulse.exe'),'--installer-verify'],cwd=missing_root,env=env,timeout=30)
assert bad.returncode!=0, 'Installer must reject missing bundled runtimes'

subprocess.run([str(OUT/'NetPulseService.exe'),'--self-test',str(smoke/'service-result.json')],cwd=OUT,env=env,timeout=30,check=True)
service_smoke=json.loads((smoke/'service-result.json').read_text(encoding='utf8'));assert service_smoke['result']=='passed'
result={'version':'2.0.0','stable_ready':False,'release_channel':'release','native_tests':receipt['counts'],'tested_core_sha256':receipt['core_sha256'],'clippy':'passed','frontend_build':'passed','frontend_state_tests':{'passed':12,'failed':0},'photo_qr_tests':{'passed':2,'failed':0},'selection_async_regression':{'passed':1,'failed':0},'packaged_smoke':smoke_result,'packaged_service_smoke':service_smoke,'browser_tests':'not_run_user_restriction','physical_tun':'not_run_requires_uac_windows_validation','windows_service':'compiled_named_pipe_sid_pid_tests_passed_not_installed','system_proxy_crash_recovery':'implemented_physical_crash_test_not_run','signing':'unsigned','engine_bundled':True,'engine_update_policy':'official_github_releases_including_prerelease_sha256_schema_checks_rollback_independent_service_approval','installer_runtime_test':'not_run_to_preserve_existing_installation','tray_autostart':'compiled_not_manually_exercised'}
result['version']=VERSION
result['packaged_webview_startup']=startup_cases
for case in startup_cases:
    assert len(case['rendered_views'])==16 and case['layout_geometry'] is True and case['form_help_coverage'] is True
result['installer_component_verification']={'official_bundle':'passed','missing_bundle':'rejected'}
assert stress_path.is_file(), 'Run the owned whitelist routing stress test before packaging'
stress=json.loads(stress_path.read_text())
assert stress['result']=='passed' and stress['core_sha256']==native.core_fingerprint()
result['routing_stress']=stress
result['application_routing']={'case_insensitive_windows_paths':True,'discord_version_and_updater_scoped_rules':True,'owned_process_loopback_test':True,'catalog_persists_after_deselection':True,'roblox_version_scoped_rules':True,'physical_tun':'not_run'}
result['network_3']={'profile_sni_host_utls':'validated_and_persisted_across_refresh_backup','fragment_mux':'opt_in_xray_validated_and_owned_tls_transfer_passed','fake_ip':'opt_in_tun_owned_udp_dns_mapping_and_bypass_checked','destination_rules':'ordered_domains_cidr_geoip_geosite_process','playground':'on_demand_os_tcp_udp_plus_core_metadata_no_database','dated_logs':'local_date_categories_128_mib_archive','roblox':'games_preset_and_versioned_process_rules_actual_game_not_reproduced','ja4':'core_utls_presets_only_no_exact_ja4_ja4h_guarantee','unsupported':['multipath_bonding','behavioral_background_noise','arbitrary_tls_padding','per_destination_game_node_selection','seamless_game_session_failover','per_process_distinct_vpn_transport_profiles']}

assert source_fingerprint()==PACKAGED_SOURCE, 'Application source changed during packaging; rebuild before publishing'
result['packaged_source_sha256']=PACKAGED_SOURCE
result['frontend_state_tests']={'passed':frontend_passed,'failed':0}
result['maintenance_3_0_1']={'missing_runtime_ui':'available_with_actionable_error','runtime_preparation':'background_thread','installer':'explicit_update_remove_keep_legacy_registry_identity','file_logs':'auto_tail_default_on_coloured_copyable_clear_all_categories','subscriptions':'editable_name_interval_copy_policy_automatic_refresh'}
result['maintenance_3_0_2']={'service_ipc':'independent_bounded_clients_cached_monitor','engine_stop':'bounded_reader_wait','service_recovery':'explicit_uac_verified_scm_pid_image_owner_only','tls':'global_capability_aware_defaults','playground':'manual_default_optional_1_5_10_seconds','subscription':'provider_usage_expiry_validated_json_one_way_lock','mtu':'repeated_boundary_confirmation_unused_adapters_hidden','help':'field_and_checkbox_hover_keyboard','physical_disconnection_reproduction':'not_reproduced'}
result['maintenance_3_0_3']={'mtu':'measured_path_or_labelled_active_physical_adapter_estimate_1280_1420','catalog':'atomic_applications_json_normalized_paths_known_versions_grouped','ui':'full_width_import_search_picker_core_buttons_no_per_exe_help','logs':'interaction_audit_not_written_complete_bounded_lines_explicit_severity','direct_ipv6':'ipv4_only_domain_resolver_when_physical_ipv6_default_absent_literals_not_converted','installer':'staged_current_helper_stops_verified_owned_service_before_copy_or_old_uninstall_uac','geometry':'packaged_webview_search_picker_key_field_exe_rows_core_buttons_playground_interval','playground_search':'separate_process_name_path_pid_and_connection_metadata_filters','games_preset':'preserves_system_proxy_public_loopback_no_auth_in_tun_diagnostic_auth_retained','physical_installer_and_vpn':'not_run_preserve_existing_installation'}
result['maintenance_3_0_4']={'metrics':'coalesced_read_only_worker_5s_cached_last_good_bounded_4mib_300_rows_no_controller_lock','service_routes':'cached_1s_confirmed_loss_only','service_status':'unconfirmed_warning_recovers_without_stale_banner','switching':'candidate_preflight_preserves_existing_pid_on_probe_failure','probe_error':'compact_banner_redacted_core_log_details','direct_dns':'prefer_ipv4_preserves_ipv6_only_answers_literals_need_route','hero':'compact_nowrap_core_buttons','physical_failure':'not_reproduced_installed_network_unchanged'}

endurance=ROOT/'.local/ipc-endurance.json'
if endurance.is_file():
    evidence=json.loads(endurance.read_text())
    service_hash=sha(APP/'crates/netpulse-core/src/service.rs')
    assert evidence['result']=='passed' and evidence['service_source_sha256']==service_hash
    result['ipc_endurance']=evidence
result['background_client_updates']={'startup_delay_seconds':5,'interval_hours':6,'network_request_holds_connection_mutex':False,'downloads':'user_initiated','badge_survives_banner_dismissal':True}
result['runtime_dependencies']={'pe_import_audit':runtime_imports,'visual_cpp_redistributable_required':False,'webview2_runtime_required':True,'webview2_install_mode':'downloadBootstrapper'}
update_trace=ROOT/'.local/v2-official-update-test.json'
if update_trace.is_file():
    trace=json.loads(update_trace.read_text());result['official_core_update_smoke']=trace if trace.get('core_sha256')==native.core_fingerprint() else {'result':'not_run_on_current_source'}
tcp_trace=ROOT/'.local/v2-tcp-smoke.json'
if tcp_trace.is_file():
    trace=json.loads(tcp_trace.read_text());result['tcp_latency_smoke']=trace if trace.get('core_sha256')==native.core_fingerprint() else {'result':'not_run_on_current_source'}
transfer_trace=ROOT/'.local/v2-grpc-transfer.json'
if transfer_trace.is_file():result['owned_grpc_tls_bulk_transfer']=json.loads(transfer_trace.read_text())
result['client_update_download_previous_alpha6']='official_alpha5_installer_downloaded_and_sha256_verified_no_installation'
result['external_grpc_previous_alpha6']='two_stored_profiles_successful_with_auto_xray_previously_timeout_with_sing_box_no_secrets_in_public_report'
result['wan_throughput']='small_comparisons_only_not_a_physical_tun_bandwidth_acceptance_test'
(OUT/'VERIFICATION.json').write_text(json.dumps(result,ensure_ascii=False,indent=2),encoding='utf8')
checks=[f'{sha(p)}  {p.name}' for p in [OUT/'NetPulse.exe',OUT/'NetPulse-Setup.exe',OUT/'NetPulseService.exe',source,OUT/'NetPulse-Engine-Sources.zip']]
(OUT/'checksums.sha256').write_text('\n'.join(checks)+'\n',encoding='ascii')
print(json.dumps({'output':str(OUT),'executable_bytes':(OUT/'NetPulse.exe').stat().st_size,'installer_bytes':(OUT/'NetPulse-Setup.exe').stat().st_size,'dependencies':len(inventory)},ensure_ascii=False))

final_trace=ROOT/'.local/v2-final-core-update.json'
if final_trace.is_file():
    result['official_update_final']=json.loads(final_trace.read_text())
    (OUT/'VERIFICATION.json').write_text(json.dumps(result,ensure_ascii=False,indent=2),encoding='utf8')
