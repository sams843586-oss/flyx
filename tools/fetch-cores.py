"""Fetch pinned core archives from their original maintainers; verify SHA256."""
import argparse, hashlib, io, json, tarfile, urllib.request, zipfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser();parser.add_argument('platform',choices=['windows','android'])
args=parser.parse_args()
assets=json.loads((ROOT/'packaging/core-assets.json').read_text())
cache=ROOT/'output/vendor/core-downloads';cache.mkdir(parents=True,exist_ok=True)
for item in assets[args.platform]:
    archive=cache/item['name']
    if not archive.exists():
        print('Downloading',item['name'],flush=True)
        with urllib.request.urlopen(item['url'],timeout=180) as response:archive.write_bytes(response.read())
    data=archive.read_bytes()
    if hashlib.sha256(data).hexdigest()!=item['sha256']:raise ValueError('SHA256 mismatch: '+item['name'])
    if args.platform=='android' and item['name'].endswith('.aar'):
        dest=ROOT/'android/app/libs/hev-socks5-tunnel.aar';dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(data);continue
    member=item['binary'];output=ROOT/item['destination'];output.parent.mkdir(parents=True,exist_ok=True)
    if item['name'].endswith('.zip'):
        with zipfile.ZipFile(io.BytesIO(data)) as z:
            name=next(n for n in z.namelist() if Path(n).name==member)
            output.write_bytes(z.read(name))
    else:
        with tarfile.open(fileobj=io.BytesIO(data)) as z:
            name=next(n for n in z.getmembers() if Path(n.name).name==member and n.isfile())
            output.write_bytes(z.extractfile(name).read())
    print('Verified',output.name,flush=True)
