# Phantun Windows Client

Phantun 是一个将 UDP 流量伪装成 TCP 的混淆工具。本项目是 Phantun 的 Windows 客户端，使用 WinDivert 在 Windows 网络层直接拦截和注入数据包，无需创建虚拟网卡。

## 与原版 Phantun 的区别

原版 Phantun 使用 Linux TUN 接口（`tokio-tun`），需要创建虚拟网卡并配置路由表。Windows 版本改用 **WinDivert**（Windows Filter Platform），直接在网络层拦截/注入 IP 数据包，**不需要虚拟网卡、不需要路由表配置、不需要 netsh**。

| 特性 | Linux 原版 | Windows 本版本 |
|------|-----------|---------------|
| 网络层方案 | TUN 虚拟网卡 | WinDivert 拦截注入 |
| 路由表 | 需要配置 | 不需要 |
| 虚拟网卡 | 需要创建 | 不需要 |
| 管理员权限 | 需要（创建 TUN） | 需要（安装 WinDivert 驱动） |
| 多核负载均衡 | `SO_REUSEPORT` | 随机端口（`SO_REUSEPORT` Windows 不支持） |

## 技术方案演变

### 第一阶段：wintun 方案（失败）

最初尝试复刻 Linux 方案，使用 `wintun` crate 在 Windows 上创建虚拟网卡：

1. **创建 wintun 适配器**：调用 `wintun::Adapter::create()`
2. **配置 IP 和路由**：通过 `netsh` 命令设置 `192.168.200.x` 网段
3. **启动 wintun Session**：开始收发数据包

**问题**：Windows 家庭版（Home）上，wintun 创建的 NDIS LWF（Lightweight Filter）适配器状态始终为 **Disconnected**，路由无法进入系统路由表（只能写入 Persistent routes，不会激活）。因此 SYN+ACK 返回时，Windows 不会将流量转发到该适配器，连接无法建立。

**对比**：WireGuard 使用私有的 WireGuardNT.sys（NDIS Miniport 驱动），适配器状态为 Connected，所以能正常工作。但 WireGuardNT 是闭源驱动，无法直接使用。

### 第二阶段：WinDivert 方案（成功）

改用 **WinDivert** 绕过 Windows TCP/IP 栈，直接在 WFP（Windows Filter Platform）网络层操作：

- **发送 SYN**：程序构造 raw IP 包（含 fake TCP 头），通过 WinDivert `send()` 注入 outbound 流量
- **接收 SYN+ACK**：WinDivert 按 filter 规则拦截所有来自 phantun server 的 TCP 包，阻止 Windows TCP/IP 栈看到它们（避免自动发 RST）
- **数据转发**：fake-tcp 栈维护序列号/确认号，把 WireGuard 的 UDP payload 封装在 TCP 数据段中发送

**核心修改**：Windows 下 fake-tcp 的源 IP 不再用虚拟 IP（`192.168.202.2`），而是通过 UDP socket 探测获取本机真实外网 IP（如 `192.168.1.23`），这样 Windows 才会从物理网卡正常转发该流量。

## 修改的文件清单

### 1. `fake-tcp/Cargo.toml`

```toml
[target.'cfg(target_os = "windows")'.dependencies]
windivert = { version = "0.7.0-beta.4", features = ["vendored"] }
```

替换原版的 `wintun = "0.5"` 依赖。

### 2. `fake-tcp/src/tun.rs`

重写 Windows 模块，从 `WindowsTun`（wintun 方案）改为 `WinDivertTun`：

- `WinDivertTun::new(remote_addr)`：创建 WinDivert handle，filter 规则为 `tcp and ip.SrcAddr == {ip} and tcp.SrcPort == {port}`
- `recv()`：通过 `recv_wait()` 循环拦截匹配的入站 TCP 包
- `send()` / `try_send()`：构造 `WinDivertPacket` 并设置 `set_outbound(true)`，注入出站流量

### 3. `phantun/src/bin/client.rs`

主要修改点：

1. **Windows tun 设备创建**：替换所有 `netsh` 配置代码，改为简单的 `WinDivertTun::new(remote_addr)`
2. **真实 IP 探测**（Windows）：
   ```rust
   let probe = std::net::UdpSocket::bind("0.0.0.0:0").unwrap();
   probe.connect("8.8.8.8:53").unwrap();
   let tun_local_addr = probe.local_addr().unwrap().ip(); // 如 192.168.1.23
   ```
   以此 IP 作为 fake-tcp 源地址传给 `Stack::new()`
3. **Worker 线程 UDP socket 绑定**（Windows）：Linux 用 `SO_REUSEPORT` 让多个 worker 绑定同一端口做负载均衡，Windows 不支持，改为绑定随机端口（`0.0.0.0:0`）

### 4. `phantun/src/utils.rs`

新增 Windows 平台实现：

- `new_udp_reuseport`：Windows 不支持 `SO_REUSEPORT`，退化为普通 `bind()`
- `udp_recv_pktinfo`：Windows 不支持 `IP_PKTINFO`，用 `recv_from()` + `local_addr()` 替代

## 运行时依赖

| 文件 | 说明 |
|------|------|
| `phantun-client.exe` | 主程序 |
| `WinDivert.dll` | WinDivert 用户态库（编译时从源码构建） |
| `WinDivert64.sys` | WinDivert 内核驱动（首次运行时自动安装，需管理员权限） |
| `phantun-client.json` | 配置文件 |

## 使用方法

### 1. 准备文件

把以下文件放在同一目录：

```
D:\phantun\
  phantun-client.exe
  WinDivert.dll
  WinDivert64.sys
  phantun-client.json
```

### 2. 配置文件

`phantun-client.json`：

```json
{
  "local": "127.0.0.1:8080",
  "remote": "120.26.71.147:65009",
  "ipv4_only": true
}
```

| 字段 | 说明 |
|------|------|
| `local` | UDP 监听地址，WireGuard 客户端连接这里 |
| `remote` | Phantun Server 地址 `IP:PORT` |
| `ipv4_only` | 仅使用 IPv4（推荐 Windows 开启） |

### 3. 启动

**必须以管理员身份运行**，因为 WinDivert 需要安装内核驱动。

```cmd
D:\phantun> set RUST_LOG=info
D:\phantun> phantun-client.exe
```

正常启动日志：

```
INFO  phantun_client > Remote address is: 120.26.71.147:65009
INFO  phantun_client > UDP listen: 127.0.0.1:8080 -> remote 120.26.71.147:65009
INFO  phantun_client > Fake TCP local IP: 192.168.1.23
INFO  phantun_client > Opening WinDivert handle for remote 120.26.71.147:65009
INFO  phantun_client > WinDivert ready -- no virtual adapter needed
```

### 4. WireGuard 配置

WireGuard 客户端的 Endpoint 指向 phantun-client 的监听地址：

```ini
[Peer]
Endpoint = 127.0.0.1:8080
```

WireGuard 连接后，phantun-client 日志会显示：

```
INFO  phantun_client > New UDP client from 127.0.0.1:xxxxx
INFO  fake_tcp       > Connection to 120.26.71.147:65009 established
```

## 已知限制

1. **仅支持 IPv4**：`ipv4_only` 必须设为 `true`
2. **需要管理员权限**：WinDivert 驱动安装需要 UAC 提升
3. **不支持自定义路由表**：所有流量都通过 fake TCP 隧道到单一 remote，不需要也不支持路由配置
4. **每次启动自动探测 IP**：程序启动时通过 UDP probe 获取本机 IP，如果网络环境变化（如切换 WiFi），需要重启程序

## 编译方法

### 环境要求

- Linux 主机（或 WSL）
- Rust + `x86_64-pc-windows-gnu` target
- MinGW-w64 交叉编译器 (`x86_64-w64-mingw32-gcc`)

### 安装 target

```bash
rustup target add x86_64-pc-windows-gnu
```

### 编译

```bash
cd phantun-client-win
cargo build --release --target x86_64-pc-windows-gnu
```

输出文件：
- `target/x86_64-pc-windows-gnu/release/phantun-client.exe`
- `target/x86_64-pc-windows-gnu/release/build/windivert-sys-*/out/WinDivert.dll`

### 获取 WinDivert64.sys

从 [WinDivert 官方 Release](https://github.com/basil00/WinDivert/releases/latest) 下载，取 `x64/WinDivert64.sys`。
