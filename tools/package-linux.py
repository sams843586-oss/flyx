"""Package Linux ELFs as .deb and an installable tarball, on any build host."""
import argparse
import gzip
import hashlib
import io
import json
from pathlib import Path
import tarfile
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument('--binaries', type=Path, required=True)
parser.add_argument('--output', type=Path, default=ROOT / 'output/Linux')
parser.add_argument('--cache', type=Path, default=ROOT / 'output/vendor/linux-cores')
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=True)
args.cache.mkdir(parents=True, exist_ok=True)
VERSION = '0.1.1'

CORES = [
    ('sing-box', 'https://github.com/SagerNet/sing-box/releases/download/v1.14.4/sing-box-1.14.4-linux-amd64.tar.gz',
     '699a1b510aa4396b2176d32d1181920399ebf1e267835da6f3b598d0f7c24968'),
    ('xray', 'https://github.com/XTLS/Xray-core/releases/download/v26.11.9/Xray-linux-64.zip',
     'b651f7e1d9966ee2c1b1ee1bf8b52c72771824d3065dce96bbc8bcc19ed95f82'),
]

files = {}
def add(name, content, mode=0o644):
    files[name] = (content, mode)

def elf(path):
    content = path.read_bytes()
    if content[:5] != b'\x7fELF\x02' or int.from_bytes(content[18:20], 'little') != 62:
        raise ValueError(f'{path}: expected Linux x86_64 ELF, not a Windows EXE')
    return content

for name, dest in [('Flyx', 'usr/bin/flyx'), ('flexconnect', 'usr/bin/flexconnect'),
                   ('flexconnect-service', 'usr/lib/flyx/flexconnect-service')]:
    add(dest, elf(args.binaries / name), 0o755)

for core, url, digest in CORES:
    archive = args.cache / url.rsplit('/', 1)[1]
    if not archive.exists():
        print('Downloading', core, flush=True)
        request = urllib.request.Request(url, headers={'User-Agent': 'Flyx-Linux-Builder/0.1'})
        with urllib.request.urlopen(request, timeout=120) as response:
            archive.write_bytes(response.read())
    content = archive.read_bytes()
    if hashlib.sha256(content).hexdigest() != digest:
        raise ValueError(f'SHA256 mismatch: {archive}; remove this cached download and retry')
    if archive.suffix == '.zip':
        with zipfile.ZipFile(io.BytesIO(content)) as source:
            add('usr/lib/flyx/' + core, source.read(core), 0o755)
            for name in source.namelist():
                if Path(name).name.lower() in ('license', 'license.txt'):
                    add('usr/share/doc/flyx-vpn/' + core + '-LICENSE', source.read(name))
    else:
        with tarfile.open(fileobj=io.BytesIO(content)) as source:
            for member in source:
                if member.isfile() and Path(member.name).name == core:
                    add('usr/lib/flyx/' + core, source.extractfile(member).read(), 0o755)
                elif member.isfile() and Path(member.name).name.lower() in ('license', 'license.txt'):
                    add('usr/share/doc/flyx-vpn/' + core + '-LICENSE', source.extractfile(member).read())

for name in ('sing-box', 'xray'):
    binary, _ = files['usr/lib/flyx/' + name]
    if binary[:4] != b'\x7fELF': raise ValueError(f'{name}: invalid core executable')

add('usr/lib/systemd/system/flyx.service', (ROOT/'packaging/linux/flyx.service').read_bytes())
add('usr/share/applications/flyx.desktop', (ROOT/'packaging/linux/flyx.desktop').read_bytes())
add('usr/share/pixmaps/flyx.png', (ROOT/'crates/app/icons/icon.png').read_bytes())
add('usr/share/doc/flyx-vpn/README.md', (ROOT/'LINUX.md').read_bytes())

def tar_bytes(entries, compression):
    stream = io.BytesIO()
    with tarfile.open(fileobj=stream, mode='w:' + compression, format=tarfile.GNU_FORMAT) as tar:
        parents = sorted({str(parent) for name in entries for parent in Path(name).parents if str(parent) != '.'}, key=lambda p:(len(Path(p).parts),p))
        for name in parents:
            info = tarfile.TarInfo(name.replace('\\', '/')); info.type = tarfile.DIRTYPE; info.mode = 0o755
            info.uid = info.gid = 0; info.uname = info.gname = 'root'; tar.addfile(info)
        for name, (content, mode) in sorted(entries.items()):
            info = tarfile.TarInfo(name); info.size = len(content); info.mode = mode
            info.uid = info.gid = 0; info.uname = info.gname = 'root'
            tar.addfile(info, io.BytesIO(content))
    return stream.getvalue()

data = tar_bytes(files, 'xz')
control = f'''Package: flyx-vpn
Version: {VERSION}
Architecture: amd64
Maintainer: Flyx
Section: net
Priority: optional
Installed-Size: {sum(len(content) for content, _ in files.values()) // 1024}
Depends: libc6 (>= 2.36), libgcc-s1, libssl3 | libssl3t64, libgtk-3-0 | libgtk-3-0t64, libwebkit2gtk-4.1-0, libayatana-appindicator3-1, ca-certificates, iproute2, nftables, systemd, xdg-utils, pkexec
Description: Flyx VPN client
 Desktop interface and privileged systemd tunnel supervisor.
'''
controls = {'control': (control.encode(), 0o644)}
for name in ('postinst','prerm','postrm'):
    controls[name] = ((ROOT/'packaging/linux'/name).read_bytes().replace(b'\r\n',b'\n'), 0o755)

def ar_member(name, content):
    header = f'{name + "/":<16}{0:<12}{0:<6}{0:<6}{"100644":<8}{len(content):<10}`\n'.encode('ascii')
    return header + content + (b'\n' if len(content) % 2 else b'')

deb = args.output / f'flyx-vpn_{VERSION}_amd64.deb'
deb.write_bytes(b'!<arch>\n' + ar_member('debian-binary',b'2.0\n')
                + ar_member('control.tar.gz',tar_bytes(controls,'gz')) + ar_member('data.tar.xz',data))

tar_files = dict(files)
tar_files['install.sh'] = ((ROOT/'packaging/linux/install.sh').read_bytes().replace(b'\r\n',b'\n'), 0o755)
tar_files['README.md'] = ((ROOT/'LINUX.md').read_bytes(), 0o644)
archive = args.output / f'Flyx-Linux-x86_64-{VERSION}.tar.gz'
archive.write_bytes(tar_bytes(tar_files,'gz'))
checksums = '\n'.join(f'{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.name}' for p in (deb,archive))+'\n'
(args.output/'SHA256SUMS').write_text(checksums)
(args.output/'README.md').write_bytes((ROOT/'LINUX.md').read_bytes())
for path in (deb, archive): print(path, path.stat().st_size, 'bytes', flush=True)

