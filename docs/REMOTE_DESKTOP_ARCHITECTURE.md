# EdgeNet 开源远程桌面架构规划

## 1. 定位与范围

EdgeNet 当前是虚拟组网与 VPN 客户端。远程桌面能力应建立在 EdgeNet 提供的安全、可达的虚拟网络之上，而不是混入网络核心。

本阶段只定义扩展边界；不新增远程桌面依赖，不实现屏幕捕获、编码、输入控制或 RDP 会话。

## 2. 分层架构

```text
EdgeNet Core
├── 网络与组网
│   ├── 本地 IP / 出口接口识别
│   ├── 公网 IP 探测
│   ├── VPN 隧道与虚拟 IP
│   └── 路由、防火墙与权限管理
│
└── Remote Interface（预留）
    ├── RDP       使用系统或第三方 RDP 客户端
    ├── View      只观看，不允许输入
    └── Control   观看并允许键盘、鼠标控制
```

远程能力通过独立 Provider 接入。未来替换或新增实现时，不应修改 VPN、路由、IP 探测等核心逻辑。

## 3. 远程接口草案

建议新增 `src/remote/`，先以统一领域模型固定 GUI、CLI 与实现层之间的契约：

```rust
pub enum RemoteMode {
    Rdp,
    View,
    Control,
}

pub trait RemoteProvider {
    fn name(&self) -> &str;
    fn is_available(&self) -> bool;

    fn connect(&self, target: &str, mode: RemoteMode) -> Result<(), String>;
}
```

模块出口：

```rust
pub mod provider;

pub use provider::{RemoteMode, RemoteProvider};
```

`View` 与 `Control` 必须从接口层区分，不能把只观看实现为 RDP 的临时开关。这样设备详情页可稳定提供 `RDP`、`观看`、`控制` 三类入口。

## 4. 双平台目标

支持 Windows 与 Ubuntu/Linux 不是只增加条件编译；平台能力需要分别实现，公共业务保持共享。

```text
                    EdgeNet Core
                         │
             ┌───────────┴───────────┐
             │                       │
          Windows                  Linux
             │                       │
   Windows Adapter / APIs     Linux Interface / Netlink
   WireGuardNT / Wintun       Linux WireGuard
   Windows Route / Firewall   Route / nftables 或 iptables
   RDP / Capture APIs         RDP client / PipeWire 等
```

当前 `UdpSocket` 的出口路由选择和 `reqwest` 的公网 IP 查询可跨平台复用。现有 Linux 输出为空是 CLI 中主动的占位逻辑，并非这些能力无法在 Linux 实现。

## 5. 推荐目录结构

```text
src/
├── main.rs
├── cli/
│   └── mod.rs
├── core/
│   ├── mod.rs
│   ├── config.rs
│   ├── status.rs
│   ├── error.rs
│   └── public_ip.rs
├── tunnel/
│   ├── mod.rs
│   └── provider.rs
├── remote/
│   ├── mod.rs
│   └── provider.rs
├── platform/
│   ├── mod.rs
│   ├── windows/
│   │   ├── mod.rs
│   │   ├── network.rs
│   │   ├── privilege.rs
│   │   ├── route.rs
│   │   ├── firewall.rs
│   │   └── tunnel.rs
│   └── linux/
│       ├── mod.rs
│       ├── network.rs
│       ├── privilege.rs
│       ├── route.rs
│       ├── firewall.rs
│       └── tunnel.rs
└── gui/
    └── mod.rs
```

这是目标结构，不要求一次性迁移完成。当前代码量较少，建议优先抽取公共状态、错误与平台接口，再逐步迁移 Windows 实现并新增 Linux 实现。

## 6. 统一平台契约

上层不应依赖具体操作系统。可先定义如下边界：

```rust
pub struct NetworkStatus {
    pub local_ip: Option<String>,
    pub public_ip: Option<String>,
    pub interface: Option<String>,
    pub virtual_ip: Option<String>,
}

pub trait PlatformNetwork {
    fn local_ip(&self) -> Result<String, String>;
    fn interface_name(&self) -> Result<String, String>;
}

pub trait TunnelProvider {
    fn connect(&self) -> Result<(), String>;
    fn disconnect(&self) -> Result<(), String>;
    fn status(&self) -> Result<TunnelStatus, String>;
}
```

Windows 与 Linux 分别实现 `PlatformNetwork`、`TunnelProvider`，GUI 与 CLI 只消费统一的状态和操作。

## 7. 平台职责

| 能力 | Windows | Ubuntu / Linux |
| --- | --- | --- |
| 本地 IP、出口网卡 | IP Helper API | netlink / 系统接口 |
| 权限 | Administrator | root 或 capabilities |
| 虚拟隧道 | WireGuardNT / Wintun | 内核 WireGuard |
| 路由 | Windows API | netlink |
| 服务 | Windows Service | systemd |
| 防火墙 | Windows Firewall | nftables / iptables |
| RDP | 系统 RDP 客户端 | RDP 客户端或兼容 Provider |
| 观看与控制 | Windows Capture 等 | PipeWire / 桌面协议等 |

## 8. 实施顺序

1. 增加 `core`、`tunnel`、`remote` 的接口与领域模型，不接入远程实现。
2. 将当前 Windows 网络探测迁入平台适配层，并将可复用逻辑提取到公共层。
3. 实现 Linux 的网络状态、权限、路由和隧道 Provider。
4. 让 CLI 与 GUI 基于统一 `NetworkStatus` 展示状态。
5. 接入 RDP Provider；随后再选择并实现只观看/控制的远程桌面引擎。

## 9. 安全原则

- 远程桌面只通过 EdgeNet 的身份、授权与虚拟网络边界访问。
- `View` 模式必须在协议和输入处理层禁止控制，而不仅是隐藏 GUI 控件。
- 证书、私钥、令牌与本机 VPN 配置不得提交到版本库。
- 平台特权操作应收敛到最小权限的适配层，并提供明确的失败信息。

## 10. 当前结论

EdgeNet 可以以双平台网络底座发展为远程桌面产品。近期重点是完成可测试的 Windows/Linux 平台抽象，远程桌面仅预留 Provider 接口；这样未来采用 Windows RDP、系统屏幕捕获或其他开源远程控制引擎时，均不需要侵入组网核心。
