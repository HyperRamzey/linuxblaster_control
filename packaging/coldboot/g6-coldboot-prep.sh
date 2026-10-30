#!/usr/bin/env bash
# Prepare the Sound BlasterX G6 once after a cold boot.
#
# Two separate firmware quirks are handled here:
#   1. The HID endpoint does not answer until the device receives a USB reset.
#   2. The first audio enumeration reports an illegal UAC clock source, which
#      only a full re-enumeration (like a physical replug) clears.
#
# udev starts the unit on the G6's add event. It is NOT ordered before the
# display manager and no target pulls it in, so it never delays boot.
# PipeWire handles the re-enumeration and re-links the downmix filter chain
# by node name.
#
# Sequence: wait for firmware -> USBDEVFS_RESET (wake HID) -> authorized
# toggle (clean audio enumeration) -> load snd-usb-audio -> verify HID ping.
#
# Needs python3 for the hidraw ping and the USBDEVFS_RESET ioctl. If python3
# is missing the reset is skipped and the device state is reported as unknown.
set -u

log() {
    if command -v logger >/dev/null 2>&1; then
        logger -t g6-coldboot -- "$*"
    else
        printf 'g6-coldboot: %s\n' "$*" >&2
    fi
}

have_python() { [ -x /usr/bin/python3 ]; }

find_g6() {
    local d
    for d in /sys/bus/usb/devices/*; do
        [ -f "$d/idVendor" ] || continue
        if [ "$(cat "$d/idVendor" 2>/dev/null)" = "041e" ] &&
           [ "$(cat "$d/idProduct" 2>/dev/null)" = "3256" ]; then
            printf '%s\n' "$d"
            return 0
        fi
    done
    return 1
}

hidraw_iface4() {
    local h
    for h in /sys/class/hidraw/hidraw*; do
        if grep -q 'input4' "$h/device/uevent" 2>/dev/null &&
           grep -q '0000041E:00003256' "$h/device/uevent" 2>/dev/null; then
            printf '/dev/%s\n' "$(basename "$h")"
            return 0
        fi
    done
    return 1
}

# 0 = answered, 1 = silent, 2 = cannot check (no python3 / no hidraw node)
ping_hid() {
    local hid
    hid=$(hidraw_iface4) || return 1
    have_python || return 2
    /usr/bin/python3 - "$hid" <<'PY'
import os, select, sys
try:
    fd = os.open(sys.argv[1], os.O_RDWR | os.O_NONBLOCK)
except OSError:
    sys.exit(1)
payload = bytearray(65)
payload[1:5] = bytes([0x5a, 0x06, 0x01, 0x01])
try:
    os.write(fd, payload)
except OSError:
    os.close(fd)
    sys.exit(1)
r, _, _ = select.select([fd], [], [], 1.0)
if not r:
    os.close(fd)
    sys.exit(1)
try:
    data = os.read(fd, 64)
except OSError:
    os.close(fd)
    sys.exit(1)
os.close(fd)
sys.exit(0 if len(data) >= 2 and data[1] == 0x06 else 1)
PY
}

reset_g6() {
    local dev="$1" bus num node
    have_python || return 1
    bus=$(printf '%03d' "$(cat "$dev/busnum" 2>/dev/null)") || return 1
    num=$(printf '%03d' "$(cat "$dev/devnum" 2>/dev/null)") || return 1
    node="/dev/bus/usb/$bus/$num"
    [ -e "$node" ] || { log "no usbfs node $node"; return 1; }
    log "USBDEVFS_RESET $node"
    /usr/bin/python3 - "$node" <<'PY'
import os, fcntl, sys
try:
    fd = os.open(sys.argv[1], os.O_WRONLY)
except OSError as e:
    print(f"open: {e}", file=sys.stderr)
    sys.exit(0)
try:
    fcntl.ioctl(fd, 0x5514, 0)
except OSError as e:
    # ENODEV is normal when the reset immediately disconnects the device.
    print(f"ioctl: {e}", file=sys.stderr)
finally:
    os.close(fd)
PY
    return 0
}

toggle_authorized() {
    local dev="$1" d
    if [ -z "$dev" ] || [ ! -e "$dev/authorized" ]; then
        for _ in $(seq 1 40); do
            dev=$(find_g6) && [ -e "$dev/authorized" ] && break
            dev=""
            sleep 0.25
        done
    fi
    [ -n "$dev" ] && [ -e "$dev/authorized" ] || return 1

    log "authorized toggle at $dev"
    echo 0 > "$dev/authorized" 2>/dev/null || true
    sleep 1

    for _ in $(seq 1 40); do
        dev=$(find_g6) || { dev=""; sleep 0.25; continue; }
        if [ -e "$dev/authorized" ]; then
            echo 1 > "$dev/authorized" 2>/dev/null || true
            [ "$(cat "$dev/authorized" 2>/dev/null || echo 0)" = "1" ] && return 0
        fi
        sleep 0.25
    done
    return 1
}

DEV=""
for _ in $(seq 1 40); do
    DEV=$(find_g6) && break
    DEV=""
    sleep 0.25
done

if [ -z "$DEV" ]; then
    log "G6 not present; nothing to do"
    exit 0
fi

# The firmware needs about ten seconds from power-on before a reset produces
# a clean audio enumeration. The HID endpoint is dead until the reset, so a
# ping cannot be used to detect readiness.
log "found G6 at $DEV; waiting for firmware"
sleep 10

OLD_NUM=$(cat "$DEV/devnum" 2>/dev/null || echo "")
case $(ping_hid; echo $?) in
    0) log "initial HID ping: ok" ;;
    2) log "initial HID ping: skipped (python3 or hidraw not available)" ;;
    *) log "initial HID ping: fail" ;;
esac

if reset_g6 "$DEV"; then
    log "reset requested"
else
    log "WARNING: reset skipped; the HID endpoint may stay silent"
fi

# A USBDEVFS_RESET normally does not re-enumerate, so give it a moment and
# then force a clean enumeration. The toggle is what clears the illegal UAC
# clock source; the reset is what wakes the HID endpoint.
sleep 2
NEW_DEV=$(find_g6) || NEW_DEV=""
NEW_NUM=""
[ -n "$NEW_DEV" ] && NEW_NUM=$(cat "$NEW_DEV/devnum" 2>/dev/null || echo "")

if [ -n "$NEW_DEV" ] && [ -n "$OLD_NUM" ] && [ "$NEW_NUM" != "$OLD_NUM" ]; then
    log "G6 re-enumerated after reset (devnum $OLD_NUM -> $NEW_NUM)"
else
    log "G6 did not re-enumerate after reset; forcing authorized toggle"
    toggle_authorized "$NEW_DEV" || log "authorized toggle failed"
fi

udevadm settle --timeout=10 2>/dev/null || true

# 99-g6-coldboot.conf holds snd-usb-audio back from the first enumeration,
# which reports an illegal UAC clock source. Load it now, after the clean
# re-enumeration, so the driver only ever probes a valid clock tree.
if ! grep -q '^snd_usb_audio ' /proc/modules 2>/dev/null; then
    if /usr/bin/modprobe snd-usb-audio 2>/dev/null; then
        log "loaded snd-usb-audio after clean re-enumeration"
    else
        log "WARNING: failed to load snd-usb-audio"
    fi
fi

HID_OK=unknown
for _ in $(seq 1 20); do
    ping_hid
    case $? in
        0) HID_OK=1; break ;;
        2) HID_OK=unknown; break ;;
        *) HID_OK=0 ;;
    esac
    sleep 0.5
done

if [ "$HID_OK" = 1 ]; then
    log "HID ping ok"
elif [ "$HID_OK" = 0 ]; then
    log "WARNING: HID ping still failing"
fi

# Publish readiness for anything that wants to know the device is settled.
# The GUI does not require this flag (it retries on its own).
date +%s > /run/g6-coldboot-ready 2>/dev/null || true

# Do not run again for a later replug: this is a cold-boot fix.
touch /run/g6-coldboot-done 2>/dev/null || true

log "G6 cold-boot preparation complete (hid_ok=$HID_OK)"
