# Phantun Client 配置文件说明

配置文件名：`phantun-client.json`，放在 exe 同目录下。

## 完整默认配置

```json
{
  "local": "127.0.0.1:8080",
  "remote": "120.26.71.147:65009",
  "ipv4_only": true
}
```

## 字段说明

| 字段 | 类型 | 必填 | Linux | Windows | 说明 |
|------|------|------|-------|---------|------|
| `local` | string | 是 | 生效 | 生效 | UDP 监听地址，WireGuard 客户端连接这个地址 |
| `remote` | string | 是 | 生效 | 生效 | Phantun Server 地址 `IP:PORT` |
| `ipv4_only` | bool | 否 | 生效 | **必须 true** | 仅使用 IPv4。Windows 版本目前只支持 IPv4 |
| `tun_local` | string | 否 | 生效 | **忽略** | TUN 网卡本端 IP（Linux 系统侧），默认 `192.168.200.1` |
| `tun_peer` | string | 否 | 生效 | **忽略** | TUN 网卡对端 IP（Linux phantun 侧），默认 `192.168.200.2` |
| `tun_local6` | string | 否 | 生效 | **忽略** | IPv6 本端地址，默认 `fcc8::1` |
| `tun_peer6` | string | 否 | 生效 | **忽略** | IPv6 对端地址，默认 `fcc8::2` |
| `routes` | array | 否 | 生效 | **忽略** | Linux 路由表配置（见下方） |

**Windows 说明**：Windows 版本使用 WinDivert 直接拦截/注入网络层数据包，不创建虚拟网卡，因此 `tun_local`、`tun_peer`、`tun_local6`、`tun_peer6`、`routes` 等字段均被忽略。Windows 下 fake-tcp 的源 IP 由程序自动探测本机真实外网 IP 获得。

## Linux routes 说明

`routes` 数组中的每条路由会在 Linux 上通过 `ip route` 自动添加到 TUN 网卡。

| 字段 | 说明 |
|------|------|
| `dest` | 目标网段，如 `0.0.0.0/0` 表示默认路由 |
| `gateway` | 网关地址，通常填 `tun_local` 的值 |

## 典型场景

### 场景 1：只转发 WireGuard 流量（推荐，通用）
WireGuard 只访问特定 IP，不需要改默认路由：
```json
{
  "local": "127.0.0.1:8080",
  "remote": "120.26.71.147:65009",
  "ipv4_only": true
}
```

### 场景 2：Linux 全局流量走 phantun
```json
{
  "local": "127.0.0.1:8080",
  "remote": "120.26.71.147:65009",
  "ipv4_only": true,
  "tun_local": "192.168.200.1",
  "tun_peer": "192.168.200.2",
  "routes": [
    {
      "dest": "0.0.0.0/0",
      "gateway": "192.168.200.1"
    }
  ]
}
```

### 场景 3：Linux 只转发特定网段
```json
{
  "local": "127.0.0.1:8080",
  "remote": "120.26.71.147:65009",
  "ipv4_only": true,
  "tun_local": "192.168.200.1",
  "tun_peer": "192.168.200.2",
  "routes": [
    {
      "dest": "10.0.0.0/8",
      "gateway": "192.168.200.1"
    },
    {
      "dest": "172.16.0.0/12",
      "gateway": "192.168.200.1"
    }
  ]
}
```
