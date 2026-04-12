#!/usr/bin/env bash
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" >/dev/null && pwd)"
arti_msrv="$(curl https://gitlab.torproject.org/tpo/core/arti/-/raw/main/flake.nix 2>/dev/null | grep "RUSTUP_TOOLCHAIN" | cut -d '"' -f 2)"
chaff_msrv="$(grep "toolchainMsrv = " "$script_dir/../flake.nix" | cut -d '"' -f 2)"

echo "Arti MSRV: $arti_msrv"
echo "Chaff MSRV: $chaff_msrv"

if [[ "$arti_msrv" != "$chaff_msrv" ]]; then
  echo "MSRV mismatch!"
  exit 1
fi
