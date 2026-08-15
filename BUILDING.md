# Building From Source

This project is Windows-first. The maintained desktop build path uses the Rust `x86_64-pc-windows-msvc` toolchain plus Node.js.

## Prerequisites

- Node.js 20 or newer
- Rust via `rustup`
- Microsoft C++ Build Tools / Visual Studio Build Tools

## Setup

```powershell
git clone https://github.com/Blur009/Blur-AutoClicker.git
cd Blur-AutoClicker
npm install
rustup default stable-x86_64-pc-windows-msvc
```

## Run in development

```powershell
npm run dev
```

## Build a release bundle

```powershell
npm run build
```

The built Windows installer is written to `src-tauri/target/release/bundle/nsis/`.

## Build the portable zip

The portable zip contains the exe plus the VC++ runtime DLLs, crashpad handler
and WebView2 bootstrapper, and ships a `portable.txt` marker that activates
portable mode at runtime. Build the release first, then:

```powershell
npm run build
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/package-portable.ps1
```

The zip is written to `BlurAutoClicker-v<version>-portable.zip` in the repo
root. Running the script locally without `-Tag` defaults the tag to `dev`,
producing `BlurAutoClicker-vdev-portable.zip`; CI passes the real tag (e.g.
`-Tag v3.9.1`) so the version in the filename is correct. Portable mode keeps
all app data (settings, stats, logs, WebView2 user data) inside a `Data/`
folder next to the exe; there is no in-app auto-update — users download new
versions from GitHub Releases.

## Validation

```powershell
npm run lint
npm run frontend:build
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo check --manifest-path src-tauri/Cargo.toml --locked
cargo test --manifest-path src-tauri/Cargo.toml --locked
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for pull request guidelines and workflow.
