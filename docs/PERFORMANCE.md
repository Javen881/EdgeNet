# 上传吞吐排查与 UDP 对比

## 当前发现与改动

原客户端在 `src/vpn.rs` 中固定 `proto tcp-client`，原生桥接固定使用 TAP，
DCO 关闭。没有发现 EdgeNet 自己的逐包转发循环；数据传输由 OpenVPN3 处理。
单凭直连 70Mbps、VPN 3Mbps 不能确定 CPU、丢包、路径、MTU 或 TCP 嵌套中的哪一项是主因。

GUI 现在支持选择 TCP 或 UDP，连接期间不可修改。默认仍为 TCP，以兼容现有服务器。
旧的 `build_profile` 调用保持 TCP；新入口 `build_profile_with_transport` 可显式选择协议。
同时修复 GUI 忽略连接启动错误、TAP 初始化失败后状态停留在 Connecting 的问题。
这些变更已完成 Windows x64 Release 交叉编译和配置生成测试，
尚未在 Windows 或真实 VPN 环境运行验证，不能视作已经恢复 70Mbps。

## 服务端准备

只有服务器已经监听 OpenVPN UDP 时才在 GUI 选择 UDP，并填写该监听端口。
TCP 和 UDP 的同一数字端口是不同的监听端点；只开放 TCP 防火墙规则不够。
请由服务器管理员确认 UDP 监听、防火墙/NAT 放行，以及客户端证书和 tls-auth 配置匹配。
如果新增并行 UDP 服务实例，应使用独立的虚拟网段，避免与现有实例冲突。
仓库不含服务端配置，本次没有更改服务器。

## 使用 iperf3 复测

以下命令适用于 **iperf3**；iperf2 与 iperf3 不能混用。
在同一台测试服务器启动 `iperf3 -s`，保持服务端、客户端、接入网络和测试时间段一致。
直连使用该服务器可直达地址，VPN 测试使用其隧道内地址，并核实路由确实经过隧道。
不要拿不同位置的公网测速站与 VPN 内网服务器直接比较。

分别在直连、VPN TCP、VPN UDP 三种状态运行，每项重复三次：

```powershell
# 客户端向服务器上传：单连接
iperf3 -c <测试服务器地址> -t 30 -O 3 --get-server-output
# 四连接上传
iperf3 -c <测试服务器地址> -t 30 -O 3 -P 4 --get-server-output
# 服务器向客户端下载
iperf3 -c <测试服务器地址> -t 30 -O 3 -R --get-server-output
```

在 VPN UDP 状态，上面的 iperf3 仍测试 TCP 应用流量，这正好用于对比隧道传输协议。
额外使用 UDP 应用流量判断丢包，逐步增加发送速率，不把发送端的目标速率当成实收吞吐：

```powershell
iperf3 -c <测试服务器地址> -u -b 10M -l 1200 -t 30
iperf3 -c <测试服务器地址> -u -b 30M -l 1200 -t 30
iperf3 -c <测试服务器地址> -u -b 60M -l 1200 -t 30
```

记录接收端 Mbps、TCP Retr（平台支持时）、UDP 丢包率/抖动、往返延迟，以及两端每个 CPU 核心的占用。
保留连接日志中的协议、协商加密算法、驱动信息，避免分享含私钥的配置。

| 现象 | 后续验证 |
| --- | --- |
| TCP 隧道慢、UDP 隧道明显快 | 检查外层 TCP 重传与路径丢包，优先使用已验证的 UDP 服务 |
| 单连接慢、四连接明显快 | 检查 RTT、TCP 窗口/重传；不能据此直接认定 CPU 不足 |
| 两种协议都慢且某一核心满载 | 确认 C++ Bridge 也用 Release 构建，再分析加密与驱动开销 |
| 两种协议都慢且 CPU 空闲 | 核查服务端出口、限速、路由、丢包和 MTU |
| 小报文正常、大报文丢失或卡顿 | 测量路径 MTU 后再调整 MTU/MSS，避免盲目修改 |

本次未直接开启 Wintun/DCO：它们涉及驱动部署、编译支持和兼容性验证。
也未随意扩大 socket buffer 或 TCP 队列；没有测量依据时，这些设置可能增加延迟。

## 构建与验证限制

按 `BUILDING.md` 准备 Rust、MinGW、固定版本 OpenVPN3/vcpkg 和自己的测试证书。
C++ Bridge 使用 `-DCMAKE_BUILD_TYPE=Release`，然后构建 GUI：

```bash
cargo build --release --target x86_64-pc-windows-gnu --bin edgenet_gui
```

在 Windows 上验证：原有 TCP 连接、已开通服务端的 UDP 连接、错误端口连接/取消、
驱动安装失败的错误显示、断开后重新连接，以及上述 iperf3 对比。

参考：[OpenVPN TCP 嵌套重传问题](https://openvpn.net/as-docs/faq-tcp-meltdown.html)、
[OpenVPN 性能排查](https://openvpn.net/as-docs/v3/tutorials/tutorial--vpn-performance-troubleshooting.html)。
