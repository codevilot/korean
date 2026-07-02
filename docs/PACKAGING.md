# Packaging

This project currently publishes an unsigned APT repository through GitHub
Pages. That is useful for fast installation, but it is not the same as being in
Debian or Ubuntu official archives.

## Current APT Repository

Build and publish locally:

```bash
./scripts/package-deb.sh
./scripts/build-apt-repo.sh dist public
```

The GitHub Actions workflow publishes the generated `public/` repository when
`main` or a `v*` tag is pushed.

Users install from the current repository with:

```bash
echo "deb [trusted=yes] https://codevilot.github.io/korean stable main" | sudo tee /etc/apt/sources.list.d/korean.list
sudo apt update
sudo apt install korean
```

Before this repository is presented as production quality, replace
`trusted=yes` with a signed archive key and publish `InRelease` or
`Release.gpg`.

## Debian And Ubuntu Archive Path

For Debian, the expected path is:

1. Check for package name conflicts.
2. Add a policy-compliant `debian/` source package directory.
3. Build a source package with `dpkg-buildpackage -S`.
4. Run `lintian` and fix warnings that affect archive acceptance.
5. File an ITP bug against WNPP.
6. Upload the source package to mentors.debian.net.
7. File an RFS bug and find a sponsor.
8. Respond to sponsor review until the package is uploaded.

For Ubuntu, the fastest public route is a Launchpad PPA:

1. Create a Launchpad account and PPA.
2. Create or import a GPG key for upload signing.
3. Build a signed source package.
4. Upload with `dput`.
5. Let Launchpad build binary packages for supported Ubuntu releases.

Getting into Ubuntu Universe directly usually happens after Debian accepts the
package, then Ubuntu syncs it during the appropriate development window.

## Useful Local Checks

```bash
cargo fmt --check
cargo test
./scripts/package-deb.sh
./scripts/build-apt-repo.sh dist public
```

After Pages deployment, verify the published package metadata:

```bash
curl -fsSL https://codevilot.github.io/korean/dists/stable/main/binary-amd64/Packages
```
