"""Fetch original sources of the exact bundled Go binaries and verify module h1 sums."""
from pathlib import Path
import base64,concurrent.futures,hashlib,io,json,os,re,subprocess,sys,urllib.request,zipfile
ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'.build/engine-sources';OUT.mkdir(parents=True,exist_ok=True);CACHE=Path(os.environ.get('NETPULSE_ENGINE_SOURCE_CACHE',ROOT/'.build/engine-source-cache'))
engines=[('sing-box','v1.14.4-netpulse.1',ROOT/'resources/cores/sing-box.exe','SagerNet/sing-box'),('xray','v26.10.21',ROOT/'resources/cores/xray.exe','XTLS/Xray-core')]
subprocess.run([sys.executable,ROOT/'scripts/build-sing-box.py','--prepare-only'],check=True)
def fetch(url,path):
 if not path.exists() and (CACHE/path.name).is_file():path.write_bytes((CACHE/path.name).read_bytes())
 if not path.exists():
  request=urllib.request.Request(url,headers={'User-Agent':'NetPulse-build/2.0'})
  with urllib.request.urlopen(request,timeout=90) as response:path.write_bytes(response.read())
 return path.read_bytes()
def escape(s):return ''.join('!'+c.lower() if c.isupper() else c for c in s)
modules={};roots=[]
for name,tag,exe,repo in engines:
 raw=exe.read_bytes();a=raw.find(b'\nmod\t');b=raw.find(b'\nbuild\t',a);assert a>=0 and b>a
 lines=raw[a:b].decode('utf8').splitlines();pending=None
 for line in lines:
  columns=line.split('\t')
  if columns[0]=='dep':
   if pending:modules[(pending[1],pending[2])]=pending[3]
   pending=columns if len(columns)>=4 and columns[3].startswith('h1:') else None
  elif columns[0]=='=>':
   if len(columns)>=4 and columns[3].startswith('h1:'):modules[(columns[1],columns[2])]=columns[3]
   pending=None
 if pending:modules[(pending[1],pending[2])]=pending[3]
 path=OUT/f'{name}-{tag}-source.tar.gz'
 if name!='sing-box':fetch(f'https://codeload.github.com/{repo}/tar.gz/refs/tags/{tag}',path)
 roots.append({'name':name,'tag':tag,'file':path.name,'sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'repository':f'https://github.com/{repo}',**({'base_tag':'v1.14.4','modified_by':'NetPulse'} if name=='sing-box' else {})})
def module(item):
 (name,version),expected=item;path=OUT/(hashlib.sha256((name+'@'+version).encode()).hexdigest()+'.zip')
 data=fetch('https://proxy.golang.org/'+escape(name)+'/@v/'+escape(version)+'.zip',path)
 digest=hashlib.sha256()
 with zipfile.ZipFile(io.BytesIO(data)) as archive:
  for filename in sorted(i.filename for i in archive.infolist() if not i.is_dir()):digest.update((hashlib.sha256(archive.read(filename)).hexdigest()+'  '+filename+'\n').encode())
 actual='h1:'+base64.b64encode(digest.digest()).decode();assert actual==expected,(name,version,'module checksum mismatch')
 return {'module':name,'version':version,'h1':expected,'archive':path.name,'sha256':hashlib.sha256(data).hexdigest()}
with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool:results=list(pool.map(module,modules.items()))
(ROOT/'release').mkdir(exist_ok=True)
manifest={'roots':roots,'modules':sorted(results,key=lambda v:v['module'])};(OUT/'manifest.json').write_text(json.dumps(manifest,indent=2),encoding='utf8')
target=ROOT/'release/NetPulse-Engine-Sources.zip'
with zipfile.ZipFile(target,'w',zipfile.ZIP_STORED) as archive:
 archive.write(OUT/'manifest.json','manifest.json')
 for root in roots:archive.write(OUT/root['file'],'roots/'+root['file'])
 for item in results:archive.write(OUT/item['archive'],'modules/'+item['archive'])
print(json.dumps({'modules':len(results),'source_archive':str(target),'bytes':target.stat().st_size}))

import shutil,tarfile
shutil.copy2(OUT/'manifest.json',ROOT/'resources/cores/source-manifest.json')
notices=ROOT/'resources/cores/licenses';notices.mkdir(exist_ok=True)
for item in roots:
 with tarfile.open(OUT/item['file']) as archive:
  for member in archive.getmembers():
   if member.isfile() and member.name.rsplit('/',1)[-1].upper().startswith(('LICENSE','NOTICE','COPYING')):
    (notices/(item['name']+'-'+member.name.replace('/','-'))).write_bytes(archive.extractfile(member).read())
for item in results:
 with zipfile.ZipFile(OUT/item['archive']) as archive:
  for name in archive.namelist():
   if name.rsplit('/',1)[-1].upper().startswith(('LICENSE','LICENCE','NOTICE','COPYING','UNLICENSE')) and not name.endswith('/'):
    (notices/(hashlib.sha256(name.encode()).hexdigest()[:16]+'-'+name.rsplit('/',1)[-1])).write_bytes(archive.read(name))
