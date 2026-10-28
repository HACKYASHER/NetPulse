"""Rebuild the explicitly marked NetPulse sing-box from verified upstream source.
Requires the exact Go toolchain in cores/netpulse-build/manifest.json.
"""
from pathlib import Path
import argparse,gzip,hashlib,io,json,os,shutil,subprocess,tarfile,urllib.request,uuid
ROOT=Path(__file__).resolve().parents[1]
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def prepare(work):
    patch=ROOT/'resources/cores/netpulse-build';manifest=json.loads((patch/'manifest.json').read_text())
    work.mkdir(parents=True,exist_ok=True);archive=work/'upstream.tar.gz'
    if not archive.is_file():
        request=urllib.request.Request(manifest['source_url'],headers={'User-Agent':'NetPulse-source-build'})
        with urllib.request.urlopen(request,timeout=120) as response:archive.write_bytes(response.read(64*1024*1024+1))
    if digest(archive)!=manifest['source_sha256']:raise RuntimeError('Upstream source checksum mismatch')
    fresh=work/('source-'+uuid.uuid4().hex);fresh.mkdir()
    with tarfile.open(archive) as source:
        for item in source.getmembers():
            if not (fresh/item.name).resolve().is_relative_to(fresh.resolve()):raise RuntimeError('Source archive path escapes work directory')
        source.extractall(fresh,filter='data')
    directory=fresh/('sing-box-'+manifest['base_tag'].removeprefix('v'))
    for name,checksum in manifest['patches'].items():
        if digest(patch/name)!=checksum:raise RuntimeError('NetPulse dependency patch checksum mismatch')
        shutil.copy2(patch/name,directory/name)
    shutil.copy2(patch/'manifest.json',directory/'NETPULSE-BUILD.json')
    return directory,manifest
def main():
    parser=argparse.ArgumentParser();parser.add_argument('--go',default=shutil.which('go'));parser.add_argument('--prepare-only',action='store_true');parser.add_argument('--work',type=Path,default=ROOT/'.build/netpulse-sing-box');parser.add_argument('--output',type=Path,default=ROOT/'resources/cores/sing-box.exe');options=parser.parse_args()
    directory,manifest=prepare(options.work.resolve())
    sources=ROOT/'.build/engine-sources'/('sing-box-v'+manifest['version']+'-source.tar.gz');sources.parent.mkdir(parents=True,exist_ok=True)
    # Deterministic source archive includes the exact go.mod/go.sum overrides.
    with sources.open('wb') as raw,gzip.GzipFile(fileobj=raw,filename='',mode='wb',mtime=0) as compressed,tarfile.open(fileobj=compressed,mode='w') as archive:
        for path in sorted(directory.rglob('*')):
            if path.is_file():
                data=path.read_bytes();info=tarfile.TarInfo('sing-box-'+manifest['version']+'/'+path.relative_to(directory).as_posix());info.size=len(data);info.mode=0o644;archive.addfile(info,io.BytesIO(data))
    if options.prepare_only:return
    if not options.go:raise RuntimeError('Install '+manifest['go']+' or pass --go')
    go=str(Path(shutil.which(options.go) or options.go).resolve())
    version=subprocess.run([go,'version'],capture_output=True,text=True,check=True).stdout
    if manifest['go']+' ' not in version:raise RuntimeError('Reproducible build requires '+manifest['go'])
    env=os.environ.copy();env.update(CGO_ENABLED='0',GOOS='windows',GOARCH='amd64',GOAMD64='v1',GOTOOLCHAIN='local',GOFLAGS='')
    result=subprocess.run([go,'build','-mod=readonly','-trimpath','-buildvcs=false','-tags',manifest['tags'],'-ldflags',manifest['ldflags'],'-o',str(options.output.resolve()),'./cmd/sing-box'],cwd=directory,env=env,capture_output=True)
    if result.returncode:raise RuntimeError(result.stderr.decode(errors='replace'))
    if digest(options.output)!=manifest['sha256']:raise RuntimeError('Built executable does not match the published reproducible hash')
    print(json.dumps({'version':manifest['version'],'sha256':digest(options.output),'source_archive':str(sources)}))
if __name__=='__main__':main()
