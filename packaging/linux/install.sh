#!/bin/sh
# Install the extracted Linux tarball; run explicitly with sudo.
set -eu
[ "$(id -u)" -eq 0 ] || { echo 'Run: sudo sh install.sh' >&2; exit 1; }
base=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
[ "$(uname -m)" = x86_64 ] || { echo 'This package requires Linux x86_64.' >&2; exit 1; }
[ -d /run/systemd/system ] || { echo 'This package requires systemd.' >&2; exit 1; }
for binary in usr/bin/flyx usr/bin/flexconnect usr/lib/flyx/flexconnect-service usr/lib/flyx/sing-box usr/lib/flyx/xray; do
    [ -f "$base/$binary" ] || { echo "Missing: $binary" >&2; exit 1; }
done
getent group flyx >/dev/null || groupadd --system flyx
install -d -m 0755 /usr/lib/flyx
for binary in usr/bin/flyx usr/bin/flexconnect usr/lib/flyx/flexconnect-service usr/lib/flyx/sing-box usr/lib/flyx/xray; do
    install -D -m 0755 "$base/$binary" "/$binary"
done
for file in usr/lib/systemd/system/flyx.service usr/share/applications/flyx.desktop usr/share/pixmaps/flyx.png; do
    install -D -m 0644 "$base/$file" "/$file"
done
if [ -n "${SUDO_USER:-}" ] && [ "$SUDO_USER" != root ] && id "$SUDO_USER" >/dev/null 2>&1; then
    usermod -a -G flyx "$SUDO_USER"
fi
systemctl daemon-reload
systemctl enable --now flyx.service
echo 'Installed. Log out and back in to activate flyx group membership, then launch Flyx.'
