# Phantun Windows Client

[中文](./README.zh.md)

This project is a Windows client implementation of [Phantun](https://github.com/dndx/phantun). Phantun is an obfuscation tool that disguises UDP traffic as TCP to bypass firewalls or NATs that only allow TCP traffic. The Windows version uses WinDivert to intercept and inject packets directly at the system network layer, without creating virtual network adapters or configuring routing tables.

## Features

- Disguises UDP traffic (e.g., WireGuard) as TCP to bypass firewall restrictions
- Uses WinDivert for network-layer packet interception and injection, no virtual adapter needed
- Supports multi-worker concurrent connection handling
- Configuration file driven, easy to start

## Quick Start

### 1. Prepare Files

Place the following files in the same directory:

```
phantun-client.exe
WinDivert.dll
WinDivert64.sys
phantun-client.json
```

### 2. Configuration

Create `phantun-client.json`:

```json
{
  "local": "127.0.0.1:8080",
  "remote": "your.server.ip:65009",
  "ipv4_only": true
}
```

| Field | Description |
|-------|-------------|
| `local` | UDP listen address, WireGuard client connects here |
| `remote` | Phantun server address `IP:PORT` |
| `ipv4_only` | Use IPv4 only, recommended on Windows |

### 3. Launch

**Must run as administrator**, as WinDivert requires kernel driver installation.

```cmd
set RUST_LOG=info
phantun-client.exe
```

On successful startup, local IP and WinDivert initialization info will be displayed.

### 4. WireGuard Configuration

Point the WireGuard client's Endpoint to phantun-client's listen address:

```ini
[Peer]
Endpoint = 127.0.0.1:8080
```

## Building

### Requirements

- Linux host or WSL
- Rust + `x86_64-pc-windows-gnu` target
- MinGW-w64 cross compiler

### Install Target

```bash
rustup target add x86_64-pc-windows-gnu
```

### Build

```bash
cargo build --release --target x86_64-pc-windows-gnu
```

Output files:
- `target/x86_64-pc-windows-gnu/release/phantun-client.exe`
- `target/x86_64-pc-windows-gnu/release/build/windivert-sys-*/out/WinDivert.dll`

WinDivert64.sys must be downloaded from [WinDivert official Release](https://github.com/basil00/WinDivert/releases/latest), take `x64/WinDivert64.sys`.

## Acknowledgements and Third-Party Components

This project is based on [dndx/phantun](https://github.com/dndx/phantun), which is dual-licensed under **MIT** or **Apache-2.0**.

This project uses the following third-party open source projects or components:

| Project/Component | Purpose | License |
|-------------------|---------|---------|
| [Phantun](https://github.com/dndx/phantun) | UDP-to-TCP obfuscation core logic | MIT / Apache-2.0 |
| [WinDivert](https://github.com/basil00/WinDivert) | Windows packet interception and injection framework | LGPLv3 |
| [Tokio](https://github.com/tokio-rs/tokio) | Asynchronous runtime | MIT |

For a complete list of dependencies, see the respective `Cargo.toml` files.

## License

This project is licensed under the **Apache-2.0** License, see [LICENSE](./LICENSE).
