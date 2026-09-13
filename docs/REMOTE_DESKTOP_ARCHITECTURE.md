# EdgeNet 开源远程桌面与双平台架构：讨论过程与决策记录

> 状态：架构决策记录（ADR）
>
> 范围：EdgeNet 组网/VPN 客户端、未来 EdgeDesk 远程桌面能力，以及 Windows + Ubuntu/Linux 的演进路径。
>
> 本文按讨论过程记录“提出问题、澄清范围、得出决策”的推理。它不是已完成功能清单：远程 Provider、Linux 隧道和远程控制能力仍属于后续实现。

## 1. 讨论背景

EdgeNet 当前是一个以 Windows 为主的虚拟组网/VPN 客户端雏形。已有 CLI、Windows 网络探测、TAP/OpenVPN3 原生桥接等代码。讨论从两个问题开始：

1. 未来要支持远程桌面、只观看和远程控制，现在是否应预留接口？
2. EdgeNet 能否在 Ubuntu 上使用；如果能，实际要改多少？

这两个问题最后指向同一件事：EdgeNet 不能继续把业务逻辑固化在 Windows 文件中。它应当演进为跨平台安全组网底座，远程桌面是构建在该底座上的可替换能力。

---

## 2. 第一轮：是否应预留远程桌面接口？

### 提出的问题

远程桌面不是当前 V0.1 的实现目标，但产品将来需要 RDP、只看屏幕、看屏幕并控制三类能力。如果等到功能开发时再考虑边界，网络层与远程实现很容易耦合。

### 推理

EdgeNet 的核心职责是让设备在可信虚拟网络内可达：管理本地/虚拟 IP、隧道、路由和权限。RDP、桌面捕获、视频编码和输入注入均应建立在该可达性之上，而不应反向侵入 VPN、IP 探测或路由核心。

产品层面也已经存在三种不同的用户意图：

```text
设备：PC-001
虚拟 IP：10.88.0.11
在线：Yes

[ RDP ]   [ 观看 ]   [ 控制 ]
```

它们不是一个连接操作的不同文案：

- `RDP`：调起系统或第三方 RDP 会话。
- `View`：仅接收画面，必须禁止键盘、鼠标输入。
- `Control`：接收画面并允许经授权的输入控制。

特别是 `View` 不能只实现为“把控制按钮隐藏”或“RDP 的临时开关”。否则授权语义无法跨 GUI、协议和日志保持一致，也无法可靠审计。

### 决策

**现在建立远程 Provider 的接口层，但 V0.1 不引入任何远程桌面依赖，也不实现远程会话。**

建议目录：

```text
src/remote/
├── mod.rs
└── provider.rs
```

建议的最小契约：

```rust
// src/remote/provider.rs
pub enum RemoteMode {
    Rdp,
    View,
    Control,
}

pub trait RemoteProvider {
    fn name(&self) -> &str;
    fn is_available(&self) -> bool;

    fn connect(
        &self,
        target: &str,
        mode: RemoteMode,
    ) -> Result<(), String>;
}
```

```rust
// src/remote/mod.rs
pub mod provider;

pub use provider::{RemoteMode, RemoteProvider};
```

这样未来以下实现都只是 Provider，不需要修改组网核心：

```text
Windows 系统 RDP
Windows Shadow / Windows Capture
PipeWire 桌面捕获
第三方开源远程控制引擎
自研屏幕编解码与输入通道
```

### 边界说明

预留接口不等于远程桌面已经完成。屏幕捕获、视频编码、输入注入、会话授权、带宽控制、录像、审计及桌面环境兼容性，都应作为后续独立工作项。

---

## 3. 第二轮：Ubuntu 能不能用？

### 先拆开问题

“Ubuntu 能用吗”必须同时回答两个层面：

1. 现有部分代码是否天然跨平台？**是。**
2. 当前项目是否已经是可用的 Ubuntu 客户端？**不是。**

Rust 的可移植性和少量公共网络逻辑可复用，不代表网卡、权限、隧道、路由、防火墙、服务管理和打包已经有 Linux 实现。

### 当前代码中可复用的依据

项目已有 Windows 条件编译边界：

```rust
// src/platform/mod.rs
#[cfg(target_os = "windows")]
pub mod windows;
```

当前 `src/platform/windows/network.rs` 里的两类逻辑本身并不完全依赖 Windows：

- `UdpSocket::connect()` 可让操作系统为目标服务器选择出口源 IP；该动作不发送实际业务数据。
- `reqwest` 查询公网 IP 服务，本身跨平台。

因此，“去指定目标时选择本地出口 IPv4”和“查询公网 IP”可抽出为公共能力。

另一方面，Linux 下 CLI 显示：

```text
Server        : -
Resolved IP   : -
Local IP      : -
Interface     : -
Public IP     : -
```

并不是 Ubuntu 不能提供这些信息，而是现有 `main.rs` 对非 Windows 目标主动打印了占位值。因此 Linux 当前不可用是实现尚未完成，不是技术不可行。

---

## 4. 第三轮：为什么不是“只改几个文件”？

“能移植”被误解为“增加 Linux 条件编译即可”，因此讨论进一步明确了真实工作量。

| 层次 | 需要统一的能力 | Windows | Ubuntu/Linux |
| --- | --- | --- | --- |
| 公共层 | 配置、状态、错误、日志、命令、服务端通信、公网 IP | 共用 | 共用 |
| 网络层 | 本地 IP、出口接口、默认路由 | IP Helper API | netlink / 系统接口 |
| 权限层 | 特权动作边界 | Administrator | root 或 capabilities |
| 隧道层 | connect / disconnect / status | WireGuardNT、Wintun 或现有桥接 | 内核 WireGuard 或 Linux Provider |
| 路由/防火墙 | 建立、清理与诊断 | Windows API / Firewall | netlink、nftables 或 iptables |
| 服务层 | 后台常驻与开机启动 | Windows Service | systemd |
| GUI | 统一界面调用业务接口 | eframe/egui + Windows 动作 | eframe/egui + Linux 动作 |
| 远程能力 | RDP、观看、控制 | RDP / Capture | RDP 客户端、PipeWire 等 |
| 交付 | 安装、升级和依赖 | .exe / 安装包 | ELF、deb；以后还要考虑 ARM/OpenWrt |

所以正确结论是：**Ubuntu 具备实现条件，且部分网络逻辑能复用；但生产级双平台支持需要一次有边界的小重构。**

---

## 5. 合并后的架构决策

将 EdgeNet 定位为跨平台组网核心；远程能力以独立 Provider 接入；CLI/GUI 只消费统一领域模型，不直接依赖 Windows 或 Linux 系统调用。

```text
                         EdgeNet Core
                              │
           ┌──────────────────┼──────────────────┐
           │                  │                  │
      公共状态/配置        Tunnel 接口       Remote 接口
           │                  │                  │
           └──────────┬───────┴───────┬──────────┘
                      │               │
               Windows 平台       Linux 平台
                      │               │
          Adapter / Route / VPN   Netlink / WG / systemd
          Firewall / RDP          Firewall / PipeWire / RDP
```

必须长期保持以下约束：

1. `core` 不导入 Windows 或 Linux 专属 API。
2. `platform/windows` 和 `platform/linux` 负责具体系统能力。
3. `tunnel` 统一连接生命周期，允许底层 VPN 实现替换。
4. `remote` 表达 `Rdp`、`View`、`Control` 的业务语义，不绑定实现。
5. CLI/GUI 读取统一状态、调用统一接口，不根据操作系统复制业务分支。

---

## 6. 目标目录与职责

```text
src/
├── main.rs
├── cli/
│   └── mod.rs                # 参数解析与命令输出
├── core/
│   ├── mod.rs
│   ├── config.rs             # 配置与持久化
│   ├── status.rs             # 跨平台状态模型
│   ├── error.rs              # 统一错误类型
│   └── public_ip.rs          # 可复用公网 IP 查询
├── tunnel/
│   ├── mod.rs
│   └── provider.rs           # 隧道抽象
├── remote/
│   ├── mod.rs
│   └── provider.rs           # RDP / View / Control 抽象
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
    └── mod.rs                # 共用 UI，只调用抽象接口
```

这不是要求一次性移动所有现有文件。当前代码量较小，优先抽取公共状态与接口，逐步迁移 Windows 逻辑并新增 Linux 实现，风险最低。

---

## 7. 首先固定的领域模型

### 网络状态

```rust
pub struct NetworkStatus {
    pub local_ip: Option<String>,
    pub public_ip: Option<String>,
    pub interface: Option<String>,
    pub virtual_ip: Option<String>,
}
```

上层读取稳定的状态对象，而不是分别解析不同平台的输出文本。

### 平台网络能力

```rust
pub trait PlatformNetwork {
    fn local_ip(&self) -> Result<String, String>;
    fn interface_name(&self) -> Result<String, String>;
}
```

`WindowsNetwork` 与 `LinuxNetwork` 分别实现它；开始阶段无需为每一个系统调用创建抽象，按实际需求再细分。

### 隧道生命周期

```rust
pub trait TunnelProvider {
    fn connect(&self) -> Result<(), String>;
    fn disconnect(&self) -> Result<(), String>;
    fn status(&self) -> Result<TunnelStatus, String>;
}
```

上层只关心连接、断开与状态，不关心底层最终是 OpenVPN、WireGuardNT、Wintun 还是 Linux 内核 WireGuard。

---

## 8. 分阶段实施路线

### 阶段 A：建立边界，不改变行为

1. 新增 `core`、`tunnel`、`remote` 模块骨架。
2. 建立 `NetworkStatus`、`TunnelProvider`、`RemoteProvider` 等公共类型。
3. 在 `platform/mod.rs` 同时预留 Windows 与 Linux 模块。
4. 不引入远程桌面依赖，不承诺远程控制可用。

验收：当前 Windows 功能路径不变，结构可以容纳 Linux 和远程 Provider。

### 阶段 B：提取公共能力

1. 将公网 IP 查询迁入 `core/public_ip.rs`。
2. 抽取通用 UDP 出口选择逻辑。
3. CLI 基于 `NetworkStatus` 拼装状态输出。
4. 统一错误和日志，不让平台函数直接承担展示职责。

验收：Windows 输出保持等价，公共层无需 `windows-sys`。

### 阶段 C：Linux 最小可用网络状态

1. 新增 `platform/linux/network.rs`。
2. 实现本地 IP、出口接口、服务器路由和公网 IP 状态。
3. 在 Ubuntu 上用真实状态替换当前的 `-` 占位符。
4. 明确非 root 用户只能诊断、不能建立隧道或修改路由时的错误。

验收：Ubuntu 的 `status` 与 `doctor` 能反映真实网络。

### 阶段 D：平台隧道 Provider

1. 明确最终采用 WireGuard、现有 OpenVPN 路径，或允许两者并存。
2. Windows 实现适配器、路由、防火墙、权限和连接生命周期。
3. Linux 实现内核 WireGuard/对应隧道、地址、路由、nftables/iptables 和 systemd。
4. 为清理、重连、幂等断开和诊断建立测试。

验收：两端获得可恢复、可诊断的 VPN 会话，而不是只连接成功一次。

### 阶段 E：远程 Provider

1. 先实现 RDP Provider，复用 EdgeNet 虚拟 IP 与设备身份信息。
2. 再评估观看/控制引擎：Windows 可考虑 Capture/Shadow，Linux 桌面侧需评估 PipeWire 与会话环境。
3. 将 `View` 与 `Control` 的差异落实到协议和输入处理，不是 GUI。
4. 增加授权、断开、审计及失败反馈。

验收：同一设备的 RDP、观看、控制具有清楚、可验证、可审计的差异。

---

## 9. 安全与交付原则

### 安全边界

- 远程访问必须建立在 EdgeNet 的设备身份、授权和虚拟网络边界之上。
- `View` 模式必须在实际输入通道被拒绝，不能只禁用按钮。
- 权限提升、路由与防火墙修改集中在平台适配层，并遵循最小权限。
- 连接、断开、授权拒绝和异常清理都要有可读诊断。

### 仓库卫生

证书、私钥、令牌和本机 VPN 配置不得提交。本仓库的忽略规则已覆盖 `vpn/` 与 `svpn/` 中的证书/密钥；发布构建产物和可再生成的第三方构建目录也应与源码分开管理。

### 平台交付

Windows 和 Ubuntu 的交付不只是不同可执行文件，还包括驱动/内核模块、安装权限、服务注册、升级、卸载和故障恢复。ARM/OpenWrt 是额外平台目标，不能假定桌面 Linux 的实现可以直接复制。

---

## 10. 最终结论

这次讨论没有得出“EdgeNet 只改很少代码就可以支持 Ubuntu 或远程桌面”的结论。实际结论是：

1. 部分 Rust 网络逻辑可复用，Ubuntu 具备实现条件。
2. 完整双平台支持需要有边界的小重构，明确公共层、平台层、隧道层和 GUI 层。
3. 远程桌面现在应预留 Provider 接口，未来以 RDP、观看、控制三种模式接入。
4. EdgeNet 的核心价值是安全组网；远程能力应是上层能力，不能反向耦合组网核心。

沿上述路线推进，可以先保持 Windows 现状，再逐步获得真实 Ubuntu 支持，以及可替换、可审计的 EdgeDesk 远程能力。
