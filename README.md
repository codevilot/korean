# Korean

`korean` is a macOS-like Korean input method for Linux. It provides a native IBus engine for two-beolsik Hangul composition and a small CLI for setup, diagnostics, and simulation.

The package name is `korean`, so the intended user install flow is:

```bash
sudo apt install korean
```

For GitHub-hosted releases, users must add this repository's APT source once before running that command.

## Install from the APT repository

After GitHub Pages is enabled for this repository and the release workflow has published the `public/` APT tree, install with:

```bash
echo "deb [trusted=yes] https://codevilot.github.io/korean stable main" | sudo tee /etc/apt/sources.list.d/korean.list
sudo apt update
sudo apt install korean
```

On the next GNOME login, the package automatically registers Korean as an IBus source. The Korean engine handles Caps Lock internally for Korean/English mode switching. To apply it immediately in the current session, run:

```bash
korean start
```

The repository is currently unsigned, so the command above uses `trusted=yes`. For a production repository, sign `dists/stable/Release` and replace `trusted=yes` with a `signed-by=` keyring entry.

## Install a local `.deb`

Build and install locally on Ubuntu/Debian:

```bash
sudo apt update
sudo apt install -y build-essential cargo rustc pkg-config libibus-1.0-dev libglib2.0-dev libevdev-dev libudev-dev dpkg-dev apt-utils
./scripts/package-deb.sh
sudo apt install ./dist/korean_0.1.2_amd64.deb
korean start
```

## Build the APT repository

```bash
./scripts/package-deb.sh
./scripts/build-apt-repo.sh dist public
```

The generated repository layout is written to `public/` and is suitable for GitHub Pages. The GitHub Actions workflow in `.github/workflows/apt.yml` runs tests, builds the `.deb`, builds the APT repository, uploads the `.deb` artifact, and deploys the APT tree to Pages when `main` or a `v*` tag is pushed, or when the workflow is run manually.

## Use

```bash
korean start
korean stop
korean status
korean doctor
korean speed 180 15
korean simulate gksrmf
korean reset
```

Keyboard repeat can be tuned at any time:

```bash
korean speed 180 15
```

The first value is the repeat delay in milliseconds, and the second value is the repeat interval in milliseconds. Larger values feel slower. The setup command accepts the same tuning:

```bash
korean setup --repeat-delay-ms 220 --repeat-interval-ms 20
```

Expected simulation:

```text
g -> ㅎ
k -> 하
s -> 한
r -> 한ㄱ
m -> 한그
f -> 한글
final: 한글
```

## Development

Install build dependencies, then run the dev engine from the working tree:

```bash
sudo apt install -y build-essential cargo rustc pkg-config libibus-1.0-dev libglib2.0-dev libevdev-dev libudev-dev
./scripts/dev-apply.sh
```

During development, tune key repeat with:

```bash
KOREAN_DEV_REPEAT_DELAY_MS=180 KOREAN_DEV_REPEAT_INTERVAL_MS=15 ./scripts/dev-apply.sh
```

For a rebuild/restart loop:

```bash
./scripts/dev-watch.sh
```

The script selects `Korean Dev` and restarts IBus. If the current session does not pick it up, select `Korean Dev` from GNOME Settings > Keyboard > Input Sources or from the top-bar input source menu.

## Design

- `korean-core` is a pure Rust two-beolsik composition engine.
- `korean-state` owns input mode transitions.
- `korean-ibus` is the IBus engine and handles Hangul preedit/commit behavior.
- `korean` is the CLI installed at `/usr/bin/korean`.
- Caps Lock Korean/English switching is handled inside the Korean IBus engine.
- `korean-capsd` is optional infrastructure for future Caps Lock tap/hold behavior.

## Notes

- The package depends on IBus and a Korean-capable font package (`fonts-noto-cjk` or `fonts-nanum`).
- The engine id is `korean`; override it for development with `KOREAN_ENGINE`.
- The IBus service defaults to `org.freedesktop.IBus.Korean`; override it for development with `KOREAN_IBUS_SERVICE`.
