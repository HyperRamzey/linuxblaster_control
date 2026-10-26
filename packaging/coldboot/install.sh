#!/usr/bin/env bash
# Install (or remove) the optional G6 cold-boot helper.
#
#   sudo ./install.sh            install
#   sudo ./install.sh --remove   uninstall
#
# The helper only acts on the device's udev "add" event, once per boot, and
# is never part of the boot transaction: it cannot delay the display manager.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)

rule=/etc/udev/rules.d/71-g6-coldboot.rules
unit=/etc/systemd/system/g6-coldboot-prep.service
script=/usr/local/libexec/g6-coldboot-prep.sh
conf=/etc/modprobe.d/99-g6-coldboot.conf

if [ "$(id -u)" != 0 ]; then
    printf 'Run this as root: sudo %s %s\n' "$0" "${1:-}" >&2
    exit 1
fi

if [ "${1:-}" = "--remove" ]; then
    rm -f "$rule" "$unit" "$script" "$conf"
    udevadm control --reload-rules
    systemctl daemon-reload
    printf 'Removed the G6 cold-boot helper.\n'
    printf 'If snd-usb-audio is still blacklisted in this boot: sudo modprobe snd-usb-audio\n'
    exit 0
fi

command -v python3 >/dev/null 2>&1 || {
    printf 'warning: python3 is not installed; the HID reset and ping will be skipped.\n' >&2
}

install -Dm755 "$here/g6-coldboot-prep.sh" "$script"
install -Dm644 "$here/g6-coldboot-prep.service" "$unit"
install -Dm644 "$here/71-g6-coldboot.rules" "$rule"
install -Dm644 "$here/99-g6-coldboot.conf" "$conf"

udevadm control --reload-rules
systemctl daemon-reload

printf 'Installed the G6 cold-boot helper:\n'
printf '  %s\n  %s\n  %s\n  %s\n' "$script" "$unit" "$rule" "$conf"
printf '\nIt runs on the next cold boot, when the G6 appears on USB.\n'
printf 'To try it now without rebooting: sudo rm -f /run/g6-coldboot-done && sudo systemctl start g6-coldboot-prep.service\n'
printf 'Note: snd-usb-audio is held back until the helper loads it.\n'
