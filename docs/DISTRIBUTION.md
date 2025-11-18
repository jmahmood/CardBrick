# CardBrick Desktop - Distribution Guide

## Building the .deb Package

### Prerequisites

1. Install cargo-deb (one-time setup):
   ```bash
   cargo install cargo-deb
   ```

### Build Process

1. **Build the release binary:**
   ```bash
   cargo build --release --bin cardbrick-desktop
   ```

2. **Create the .deb package:**
   ```bash
   cargo deb --no-build
   ```

   The package will be created at:
   ```
   target/debian/cardbrick-desktop_0.1.0-1_amd64.deb
   ```

### Package Information

- **Package Name:** cardbrick-desktop
- **Version:** 0.1.0-1
- **Architecture:** amd64 (x86_64)
- **Size:** ~4.7 MB
- **Section:** education
- **License:** GPL-3.0-or-later

### What's Included

The package installs:
- **Binary:** `/usr/bin/cardbrick-desktop` (executable)
- **Desktop Entry:** `/usr/share/applications/cardbrick-desktop.desktop` (application launcher)
- **Documentation:** `/usr/share/doc/cardbrick-desktop/README.md`

## Installing the Package

### For End Users

**Ubuntu/Debian systems:**
```bash
sudo dpkg -i cardbrick-desktop_0.1.0-1_amd64.deb
```

If there are dependency issues:
```bash
sudo apt-get install -f
```

**Launch the application:**
- From the command line:
  ```bash
  cardbrick-desktop --deck /path/to/deck.cbdeck.json
  ```
- From the application menu: Search for "CardBrick Desktop" in your launcher

### Uninstalling

```bash
sudo apt-get remove cardbrick-desktop
```

## Distribution Options

### 1. Direct Download

Upload the `.deb` file to:
- GitHub Releases
- Your website
- File sharing service

Users can download and install with `sudo dpkg -i`.

### 2. Personal Package Archive (PPA)

For easier updates, create a PPA on Launchpad:

1. Create account at https://launchpad.net
2. Create a PPA (e.g., `ppa:yourname/cardbrick`)
3. Upload the package using `dput`

Users would then install with:
```bash
sudo add-apt-repository ppa:yourname/cardbrick
sudo apt-get update
sudo apt-get install cardbrick-desktop
```

### 3. GitHub Releases

1. Create a new release on GitHub
2. Upload the `.deb` file as a release asset
3. Users can download directly from releases page

### 4. Debian Repository

Host your own APT repository:

1. Set up repository structure
2. Sign packages with GPG
3. Host on a web server
4. Users add your repository to their sources

## Updating the Package

When releasing a new version:

1. Update version in `Cargo.toml`:
   ```toml
   version = "0.2.0"
   ```

2. Rebuild:
   ```bash
   cargo build --release --bin cardbrick-desktop
   cargo deb --no-build
   ```

3. New package will be: `cardbrick-desktop_0.2.0-1_amd64.deb`

## Customization

### Update Package Metadata

Edit the `[package.metadata.deb]` section in `Cargo.toml`:

```toml
[package.metadata.deb]
name = "cardbrick-desktop"
maintainer = "Your Name <your.email@example.com>"
copyright = "2025, Your Name"
extended-description = """
Your custom description here...
"""
```

### Add More Assets

```toml
assets = [
    ["target/release/cardbrick-desktop", "usr/bin/", "755"],
    ["assets/desktop/cardbrick-desktop.desktop", "usr/share/applications/", "644"],
    ["assets/icon.png", "usr/share/pixmaps/cardbrick.png", "644"],
    ["README.md", "usr/share/doc/cardbrick-desktop/", "644"],
]
```

### Dependencies

The package automatically detects runtime dependencies (`depends = "$auto"`).

To add manual dependencies:
```toml
depends = "libc6 (>= 2.35), libfontconfig1"
```

## Verification

Check package contents:
```bash
dpkg-deb --contents target/debian/cardbrick-desktop_0.1.0-1_amd64.deb
```

Check package info:
```bash
dpkg-deb --info target/debian/cardbrick-desktop_0.1.0-1_amd64.deb
```

Test installation in a VM before distribution:
```bash
# In a Ubuntu VM
sudo dpkg -i cardbrick-desktop_0.1.0-1_amd64.deb
cardbrick-desktop --deck test.cbdeck.json
```

## Support Multiple Architectures

To build for ARM64 (useful for ARM-based Linux systems):

1. Install cross-compilation tools:
   ```bash
   rustup target add aarch64-unknown-linux-gnu
   sudo apt-get install gcc-aarch64-linux-gnu
   ```

2. Build:
   ```bash
   cargo build --release --target aarch64-unknown-linux-gnu --bin cardbrick-desktop
   ```

3. Update Cargo.toml to specify target in assets path, or build separate packages.

## Troubleshooting

**Q: Package won't install due to dependencies**
- Run: `sudo apt-get install -f` to fix dependencies

**Q: Application doesn't appear in launcher**
- Run: `update-desktop-database`
- Log out and back in

**Q: Binary is too large**
- Strip debug symbols: `strip target/release/cardbrick-desktop`
- Enable LTO in Cargo.toml for smaller binaries

## License

This package uses GPL-3.0-or-later license. Make sure to include the LICENSE file in your distribution.
