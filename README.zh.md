# Phantun Windows Client

[English](./README.md)

本项目是 [Phantun](https://github.com/dndx/phantun) 的 Windows 客户端实现。Phantun 是一个将 UDP 流量伪装成 TCP 的混淆工具，用于绕过仅允许 TCP 流量的防火墙或 NAT。Windows 版本使用 WinDivert 在系统网络层直接拦截和注入数据包，无需创建虚拟网卡或配置路由表。

## 特性

- 将 WireGuard 等 UDP 流量伪装为 TCP，绕过防火墙限制
- 基于 WinDivert 实现网络层数据包拦截与注入，无需虚拟网卡
- 支持多 Worker 并发处理连接
- 配置文件驱动，启动简单

## 快速开始

### 1. 准备文件

将以下文件放在同一目录：

```
phantun-client.exe
WinDivert.dll
WinDivert64.sys
phantun-client.json
```

### 2. 配置文件

创建 `phantun-client.json`：

```json
{
  "local": "127.0.0.1:8080",
  "remote": "your.server.ip:65009",
  "ipv4_only": true
}
```

| 字段 | 说明 |
|------|------|
| `local` | UDP 监听地址，WireGuard 客户端连接这里 |
| `remote` | Phantun 服务端地址 `IP:PORT` |
| `ipv4_only` | 仅使用 IPv4，Windows 下建议开启 |

### 3. 启动

**必须以管理员身份运行**，因为 WinDivert 需要安装内核驱动。

```cmd
set RUST_LOG=info
phantun-client.exe
```

正常启动后会显示本地 IP 和 WinDivert 初始化信息。

### 4. WireGuard 配置

将 WireGuard 客户端的 Endpoint 指向 phantun-client 的监听地址：

```ini
[Peer]
Endpoint = 127.0.0.1:8080
```

## 编译

### 环境要求

- Linux 主机或 WSL
- Rust + `x86_64-pc-windows-gnu` target
- MinGW-w64 交叉编译器

### 安装 target

```bash
rustup target add x86_64-pc-windows-gnu
```

### 编译

```bash
cargo build --release --target x86_64-pc-windows-gnu
```

输出文件：
- `target/x86_64-pc-windows-gnu/release/phantun-client.exe`
- `target/x86_64-pc-windows-gnu/release/build/windivert-sys-*/out/WinDivert.dll`

WinDivert64.sys 需从 [WinDivert 官方 Release](https://github.com/basil00/WinDivert/releases/latest) 下载 `x64/WinDivert64.sys`。

## 致谢与第三方组件

本项目基于 [dndx/phantun](https://github.com/dndx/phantun) 修改而来，原项目采用 **MIT** 或 **Apache-2.0** 双许可证。

本项目使用以下第三方开源项目或组件：

| 项目/组件 | 用途 | 许可证 |
|-----------|------|--------|
| [Phantun](https://github.com/dndx/phantun) | UDP 伪装 TCP 核心逻辑 | MIT / Apache-2.0 |
| [WinDivert](https://github.com/basil00/WinDivert) | Windows 数据包拦截与注入框架 | LGPLv3 |
| [Tokio](https://github.com/tokio-rs/tokio) | 异步运行时 | MIT |

完整依赖列表见各 `Cargo.toml` 文件。

## 许可证

本项目采用 **Apache-2.0** 许可证，详见 [LICENSE](./LICENSE)。
