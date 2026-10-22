"""Fetch the official Qt SDK from a listed Qt mirror using Windows' TLS trust.

SDK archives are verified against their published SHA-256 (or SHA-1 when that
is the only checksum published for this Qt version). No installer is executed.
"""
from pathlib import Path
import concurrent.futures, hashlib, json, urllib.request, xml.etree.ElementTree as ET
import py7zr

ROOT=Path(__file__).resolve().parents[1]
CACHE=ROOT/'.tools/downloads'
CACHE.mkdir(parents=True,exist_ok=True)
BASE='https://ftp.fau.de/qtproject/online/qtsdkrepository/windows_x86/desktop/qt6_683/qt6_683/'
def get(url):
    with urllib.request.urlopen(url,timeout=90) as response:return response.read()
xml=get(BASE+'Updates.xml')
package=next(p for p in ET.fromstring(xml).findall('PackageUpdate') if p.findtext('Name')=='qt.qt6.683.win64_msvc2022_64')
archives=[a.strip() for a in package.findtext('DownloadableArchives').split(',') if a.strip().startswith(('qtbase-','qtdeclarative-','qtsvg-','qttools-'))]
def fetch(name):
    url=BASE+package.findtext('Name')+'/'+package.findtext('Version')+name
    algorithm='sha256'
    try:checksum=get(url+'.sha256').decode().split()[0]
    except Exception:
        algorithm='sha1';checksum=get(url+'.sha1').decode().split()[0]
    path=CACHE/name
    if not path.exists() or hashlib.new(algorithm,path.read_bytes()).hexdigest()!=checksum:
        print('Downloading '+name.split('-')[0],flush=True)
        with urllib.request.urlopen(url,timeout=120) as source,path.open('wb') as target:
            while block:=source.read(1024*1024):target.write(block)
    assert hashlib.new(algorithm,path.read_bytes()).hexdigest()==checksum, 'Qt archive checksum mismatch'
    return path,{'url':url,'algorithm':algorithm,'checksum':checksum,'sha256':hashlib.sha256(path.read_bytes()).hexdigest()}
with concurrent.futures.ThreadPoolExecutor(4) as pool:packages=list(pool.map(fetch,archives))
for path,record in packages:
    print('Extracting '+path.name.split('-')[0],flush=True)
    with py7zr.SevenZipFile(path) as archive:archive.extractall(ROOT/'.tools/Qt')
sdk=ROOT/'.tools/Qt'
assert (sdk/'bin/qmake.exe').is_file()
(sdk/'bin/qt.conf').write_text('[Paths]\nPrefix=..\n',encoding='utf-8')
(ROOT/'.tools/qt-provenance.json').write_text(json.dumps([r for _,r in packages],indent=2),encoding='utf-8')
print('Qt SDK ready: '+str(sdk),flush=True)
