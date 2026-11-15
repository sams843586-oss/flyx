"""Create the signed updater manifest from the actual release artifacts."""
import argparse,json,datetime
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--version',default='0.1.0');p.add_argument('--dir',type=Path,default=Path('output/Release'));args=p.parse_args()
base=f'https://github.com/sams843586-oss/flyx/releases/download/v{args.version}/'
platforms={}
for target,pattern in [('windows-x86_64','*setup.exe'),('linux-x86_64','*.deb')]:
    files=list(args.dir.glob(pattern))
    if len(files)!=1:raise ValueError(f'Expected one {target} installer, got {files}')
    file=files[0];signature=Path(str(file)+'.sig').read_text().strip()
    platforms[target]={'url':base+file.name,'signature':signature}
manifest={'version':args.version,'notes':'Flyx: Windows and Linux installers, signed updates, nine languages.',
          'pub_date':datetime.datetime.now(datetime.timezone.utc).isoformat(),'platforms':platforms}
(args.dir/'latest.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf-8')
print('Generated updater manifest:',', '.join(platforms))
