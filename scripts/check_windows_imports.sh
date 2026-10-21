#!/usr/bin/env bash
# Fails when an exe binds a DLL a Windows machine may not have: the runtime that
# `.cargo/config.toml` links statically, and the shader compiler DX12 needs, which
# is delay-loaded and must stay out of the import table.
#
# Usage: scripts/check_windows_imports.sh path/to/citerack.exe

set -euo pipefail

exe=${1:?usage: check_windows_imports.sh path/to/citerack.exe}

if ! command -v llvm-objdump >/dev/null 2>&1; then
    echo "llvm-objdump is not installed, so the imports cannot be read" >&2
    exit 2
fi

imports=$(llvm-objdump -p "$exe" |
    grep -i 'DLL Name' |
    sed 's/.*DLL Name: *//' |
    tr -d '\r' |
    sort -u)

echo "$imports"

missing=$(echo "$imports" |
    grep -iE '^(vcruntime|msvcp|msvcr|ucrtbase|d3dcompiler|concrt|mfc|api-ms-win-crt)' ||
    true)
if [ -n "$missing" ]; then
    echo "The exe binds a library a Windows machine may not have:" >&2
    echo "$missing" >&2
    exit 1
fi
