# 1.1 Installation

## Prerequisites

No prerequisites required for standard installation. The install script handles everything automatically.

### For Custom Installation (Building from Source)

- [Rust and Cargo](https://rust-lang.org/) installed

### Optional: Python Support

**Python is NOT included by default.** Vizier can be built with or without Python interpreter support:

- **Default build**: No Python dependency required
- **With Python**: Add `--features python` flag to enable Python interpreter tool

If you want to use the Python interpreter tool, you need:
1. Python 3.9+ installed on your system
2. Build Vizier with Python feature enabled

**Installing Python 3.9+ (only if you need Python support):**

**macOS:**
```sh
brew install python@3.9
```

**Ubuntu/Debian:**
```sh
sudo apt-get install python3.9 python3.9-dev
```

**Windows:**
Download from [python.org](https://www.python.org/downloads/)

## Installing Vizier

### Standard Installation (Recommended)

```sh
curl -fsSL https://get.vizier.rs | sh
```

### Alternative: Cargo Installation

If you prefer to build from source or need custom features:

```sh
cargo install vizier
```

Or using cargo-binstall (faster):
```sh
cargo binstall vizier
```

### With Python Support

```sh
cargo install vizier --features python
```

## Building from Source

Clone the repository and build manually:

```sh
git clone https://github.com/vizier-lab/vizier
cd vizier
cargo build --release
```

With Python support:
```sh
cargo build --release --features python
```

## Update Installed Version

### Using Install Script

Simply re-run the install script to get the latest version:
```sh
curl -fsSL https://get.vizier.rs | sh
```

### Using Cargo (if installed via cargo)

1. Install `cargo-update` if you haven't already:
   ```sh
   cargo install cargo-update
   ```

2. Update the binary:
   ```sh
   cargo install-update vizier
   ```
