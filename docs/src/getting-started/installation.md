# 1.1 Installation

## Prerequisites

No prerequisites required for standard installation. The install script handles everything automatically.

### For Custom Installation (Building from Source)

- [Rust and Cargo](https://rust-lang.org/) installed

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

## Building from Source

Clone the repository and build manually:

```sh
git clone https://github.com/vizier-lab/vizier
cd vizier
cargo build --release
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
