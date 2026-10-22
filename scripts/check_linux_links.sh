#!/usr/bin/env bash
# Fails when the Linux binary links a library a desktop does not always have. The
# window stack is loaded at run time with dlopen, so anything here beyond the C
# library, libm and the GCC unwinder is a dependency the README does not promise.
#
# Usage: scripts/check_linux_links.sh path/to/citerack

set -euo pipefail

bin=${1:?usage: check_linux_links.sh path/to/citerack}

if ! command -v readelf >/dev/null 2>&1; then
    echo "readelf is not installed, so the links cannot be read" >&2
    exit 2
fi

linked=$(readelf -d "$bin" |
    grep -E 'NEEDED' |
    sed 's/.*\[\(.*\)\]/\1/' |
    sort -u)

echo "$linked"

allowed='^(libc\.so\.6|libm\.so\.6|libgcc_s\.so\.1|ld-linux.*)$'
extra=$(echo "$linked" | grep -vE "$allowed" || true)
if [ -n "$extra" ]; then
    echo "The binary links a library the README does not account for:" >&2
    echo "$extra" >&2
    exit 1
fi
