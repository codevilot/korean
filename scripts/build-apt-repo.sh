#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

deb_dir="${1:-dist}"
repo_dir="${2:-public}"
codename="${CODENAME:-stable}"
component="${COMPONENT:-main}"
arch="${ARCH:-amd64}"

rm -rf "$repo_dir"
mkdir -p "$repo_dir/pool/$component/k/korean" "$repo_dir/dists/$codename/$component/binary-$arch"
cp "$deb_dir"/korean_*_$arch.deb "$repo_dir/pool/$component/k/korean/"

(
  cd "$repo_dir"
  dpkg-scanpackages --arch "$arch" "pool" >"dists/$codename/$component/binary-$arch/Packages"
  gzip -9c "dists/$codename/$component/binary-$arch/Packages" >"dists/$codename/$component/binary-$arch/Packages.gz"
  apt-ftparchive \
    -o "APT::FTPArchive::Release::Suite=$codename" \
    -o "APT::FTPArchive::Release::Codename=$codename" \
    release "dists/$codename" >"dists/$codename/Release"
)

echo "$repo_dir"
