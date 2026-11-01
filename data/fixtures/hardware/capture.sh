#!/bin/sh
# Copies the files and command output rigcheck's hardware detection reads into
# a new fixture directory, so tests can replay this machine.
#
# Usage: ./capture.sh <fixture-name>     e.g. ./capture.sh nvidia-desktop
#
# Nothing is uploaded. Look through the output before committing it.
set -eu

[ $# -eq 1 ] || { echo "usage: $0 <fixture-name>" >&2; exit 1; }
out="$(dirname "$0")/$1"

copy() {
    [ -r "$1" ] || return 0
    mkdir -p "$out$(dirname "$1")"
    cp "$1" "$out$1"
}

copy /proc/cpuinfo
copy /proc/meminfo
copy /proc/sys/kernel/osrelease
copy /etc/os-release
copy /sys/module/nvidia/version

for entry in /sys/class/drm/*; do
    case "$(basename "$entry")" in
    card*-* | renderD*) # connectors and render nodes: detection must skip these
        copy "$entry/device/uevent" ;;
    card*)
        for file in vendor device uevent mem_info_vram_total; do
            copy "$entry/device/$file"
        done ;;
    esac
done

mkdir -p "$out/commands"
if command -v vulkaninfo >/dev/null; then
    vulkaninfo --summary >"$out/commands/vulkaninfo.txt" 2>/dev/null || true
fi
if command -v nvidia-smi >/dev/null; then
    nvidia-smi --query-gpu=pci.device_id,memory.total --format=csv,noheader,nounits \
        >"$out/commands/nvidia-smi.txt" || true
fi

echo "captured into $out"
