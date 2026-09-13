# EdgeNet 构建与发布过程记录

> 适用范围：从全新 Ubuntu 开发机拉取 EdgeNet 源码，构建 Linux 开发版或 Windows x64 发布版。
>
> 当前状态：Windows x64 是已走通的主交付目标；Ubuntu 原生构建仅适用于当前 CLI/开发验证，尚不等同于完整 Linux VPN 客户端。

本文保留项目从“Ubuntu 上安装 Rust”到“Windows x64 交叉编译”的实际过程，并补充当前版本新增的 OpenVPN3、C++ 原生桥接、vcpkg 和 TAP 驱动步骤。后续请以本文作为构建入口，不要直接依赖机器上遗留的 `target/`、`native/bridge/build-*/` 或 `dist/` 文件。

## 1. 先理解当前构建产物

EdgeNet 现在同时有两层构建：

```text
Rust CLI / GUI
    cargo build --target x86_64-pc-windows-gnu
             │
             ├── build.rs：嵌入 TAP 驱动文件
             └── 链接 libedgenet_vpn.a
                         │
                         └── CMake + MinGW-w64 编译 C++ VPN Bridge
                                   │
                                   ├── OpenVPN3 源码
                                   └── vcpkg 提供的 C/C++ 依赖
```

Windows 发布目标是：

```text
target/x86_64-pc-windows-gnu/release/edgenet.exe
```

不要混淆它和 Ubuntu 原生构建产物：

```text
target/release/edgenet
```

后者是 Linux ELF，没有 `.exe`，不能拷到 Windows 运行。

## 2. 历史过程摘要

项目最初按以下顺序验证：

1. 在 Ubuntu 安装 Rust，创建最小 `rust-test`。
2. `cargo build --release` 成功，但产物是 Linux ELF。
3. 安装 MinGW-w64 并添加 Rust Windows GNU target。
4. 用 `cargo build --release --target x86_64-pc-windows-gnu` 产出 `rust-test.exe`。
5. 用 `file` 确认产物为 `PE32+`，并在 Windows 的 CMD 中验证，而不是通过双击判断。
6. 创建 EdgeNet CLI，逐步加入 Windows 网络检测、公网 IP 查询、GUI 和 VPN 原生桥接。
7. 当前版本在 Rust 之上新增静态 C++ VPN Bridge，因此完整构建需要后文的 OpenVPN3/vcpkg/CMake 步骤。

这个过程解释了一个常见现象：只执行 `cargo build --release` 永远只会得到 Linux 可执行文件；Windows 文件必须显式指定 Windows target。

## 3. 前置条件

### 3.1 Ubuntu 系统依赖

以下命令适用于 Ubuntu/Debian 开发机：

```bash
sudo apt update
sudo apt install -y \
  build-essential \
  curl \
  git \
  pkg-config \
  cmake \
  ninja-build \
  gcc-mingw-w64-x86-64 \
  g++-mingw-w64-x86-64 \
  mingw-w64 \
  file
```

检查交叉编译器：

```bash
x86_64-w64-mingw32-gcc --version
x86_64-w64-mingw32-g++ --version
```

### 3.2 Rust

如果机器尚未安装 Rust：

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

添加 Windows GNU 目标：

```bash
rustup target add x86_64-pc-windows-gnu
rustc --version
cargo --version
rustup target list --installed
```

预期已安装列表至少包含：

```text
x86_64-unknown-linux-gnu
x86_64-pc-windows-gnu
```

### 3.3 Windows 兼容性

当前 `x86_64-pc-windows-gnu` 面向 64 位 Windows。请在目标机器的 CMD/PowerShell 中运行程序并查看报错，不要只双击判断；双击控制台程序会在退出时关闭窗口。

若需支持旧版 Windows 工控机，应先确认系统版本与 Rust target 兼容性；不要默认把当前 Windows x64 产物部署到 Windows 7 之类的旧系统。

## 4. 拉取主仓库

```bash
git clone git@github.com:Javen881/EdgeNet.git
cd EdgeNet
git status --short --branch
```

本文假设当前目录就是仓库根目录。后续命令均在此目录执行。

## 5. 还原未纳入主仓库的第三方依赖

为了避免把大型第三方源码、vcpkg 安装目录和编译缓存提交到 Git，主仓库刻意忽略了以下目录：

```text
native/openvpn3/
native/vcpkg/
native/bridge/build/
native/bridge/build-static/
```

首次拉取后需要自行准备它们。

### 5.1 OpenVPN3

```bash
git clone https://github.com/OpenVPN/openvpn3.git native/openvpn3
git -C native/openvpn3 checkout c33d2c44e60cc3dc50b2e2c6128aecd4e11c8ff4
```

当前本机成功构建目录所依据的 OpenVPN3 提交为上面的 `c33d2c44`。OpenVPN3 工作区曾用于本地调试，存在未提交的调试性改动；这些改动**没有**纳入 EdgeNet 主仓库，也不应被当作正式依赖。后续如果确实证明某个补丁是构建或运行必需条件，应把它整理为 EdgeNet 的版本化补丁，而不是只留在个人工作区。

### 5.2 vcpkg

```bash
git clone https://github.com/microsoft/vcpkg.git native/vcpkg
git -C native/vcpkg checkout a1cae005c39be7b18ba319fced856b68d7276271
./native/vcpkg/bootstrap-vcpkg.sh
```

当前已验证的 vcpkg 提交是 `a1cae005`。固定提交能降低端口版本变化导致的构建漂移。

## 6. 构建 OpenVPN3 的 MinGW 依赖

OpenVPN3 仓库自带 CMake preset 与 vcpkg manifest。先设置 vcpkg 根目录：

```bash
export VCPKG_ROOT="$PWD/native/vcpkg"
```

然后配置并构建 OpenVPN3 的 x64 MinGW Release 目录：

```bash
cmake --preset mingw-x64-release -S native/openvpn3
cmake --build native/openvpn3/build/mingw/x64 --parallel
```

该 preset 使用：

```text
C 编译器：x86_64-w64-mingw32-gcc-posix
C++ 编译器：x86_64-w64-mingw32-g++-posix
triplet：x64-mingw-dynamic
构建目录：native/openvpn3/build/mingw/x64
```

它会通过 OpenVPN3 的 manifest 准备 `asio`、`fmt`、`jsoncpp`、`lz4`、`openssl`、`tap-windows6` 和 `xxhash` 等依赖。首次构建耗时较长，且会下载源码包。

## 7. 构建 EdgeNet 静态 C++ VPN Bridge

Rust 的 `build.rs` 会链接：

```text
native/bridge/build-static/libedgenet_vpn.a
```

因此必须先构建该静态库。当前项目的链接规则期待 `x64-mingw-static` vcpkg 依赖。安装所需库：

```bash
./native/vcpkg/vcpkg install \
  asio \
  fmt \
  jsoncpp \
  lz4 \
  openssl \
  --triplet x64-mingw-static
```

配置并构建 Bridge：

```bash
cmake -S native/bridge -B native/bridge/build-static -G Ninja \
  -DCMAKE_SYSTEM_NAME=Windows \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_C_COMPILER=x86_64-w64-mingw32-gcc \
  -DCMAKE_CXX_COMPILER=x86_64-w64-mingw32-g++ \
  -DCMAKE_TOOLCHAIN_FILE="$PWD/native/vcpkg/scripts/buildsystems/vcpkg.cmake" \
  -DVCPKG_TARGET_TRIPLET=x64-mingw-static \
  -DVCPKG_APPLOCAL_DEPS=OFF

cmake --build native/bridge/build-static --parallel
```

确认静态库存在：

```bash
test -f native/bridge/build-static/libedgenet_vpn.a
ls -lh native/bridge/build-static/libedgenet_vpn.a
```

### 当前已知配置约束

`native/bridge/CMakeLists.txt` 当前还引用 OpenVPN3 动态 triplet 的 include 目录：

```text
native/openvpn3/build/mingw/x64/vcpkg_installed/x64-mingw-dynamic/include
```

因此第 6 节不能跳过，即使最终 Rust 链接的是静态库。这个静态/动态依赖并存是当前构建设计的技术债；后续应统一 triplet 并把路径改成 CMake/vcpkg 的可配置变量。

## 8. 准备本机敏感 VPN 开发资产

`src/vpn.rs` 目前使用 `include_bytes!` 嵌入下列文件：

```text
vpn/ca.crt
vpn/EdgeNet-Client01.crt
vpn/EdgeNet-Client01.key
vpn/ta.key
```

它们包含证书/私钥或敏感配置，已被 `.gitignore` 排除，绝不能提交到 GitHub。新的开发机在编译前必须通过安全渠道获取自己的开发/测试资产并放在上述路径。

不要复制生产私钥；建议每个开发环境使用独立、可吊销的测试客户端身份。缺少这些文件时，Rust 编译会在 `include_bytes!` 阶段失败，这是预期的安全保护，而不是 Cargo 故障。

## 9. TAP 驱动资源

Rust 的 `build.rs` 会把 `dist/windows/drivers/tap/` 下的所有文件嵌入可执行文件。主仓库保留了当前所需的：

```text
OemVista.inf
tap0901.cat
tap0901.sys
tapinstall.exe
```

构建前检查目录非空：

```bash
find dist/windows/drivers/tap -maxdepth 1 -type f -printf '%f\n' | sort
```

不要把整个 `dist/windows/` 目录当作源代码提交；发布 EXE、DLL 和测试文件应由构建/发布流程产生。上述 TAP 文件是当前 `build.rs` 的明确输入，不能随意删除。

## 10. 编译 Rust

### 10.1 格式与依赖检查

```bash
cargo fmt --check
cargo check --target x86_64-pc-windows-gnu
```

当前历史代码可能尚未完全符合新版 `rustfmt` 输出。格式检查失败时，先确认是纯格式差异还是编译问题；不要为了打包任务批量重排无关代码。

### 10.2 Windows x64 Release

```bash
cargo build --release --target x86_64-pc-windows-gnu
```

产物：

```text
target/x86_64-pc-windows-gnu/release/edgenet.exe
```

验证文件格式与链接结果：

```bash
file target/x86_64-pc-windows-gnu/release/edgenet.exe
x86_64-w64-mingw32-objdump -p \
  target/x86_64-pc-windows-gnu/release/edgenet.exe \
  | grep "DLL Name"
```

`file` 应显示 `PE32+`、`x86-64`、`MS Windows`。如果依赖列表包含 `libgcc_s_seh-1.dll`、`libwinpthread-1.dll` 或第三方 DLL，发布时必须一并提供，或者先完成静态链接方案验证。

### 10.3 Ubuntu 原生开发构建

```bash
cargo build --release
```

产物位于 `target/release/edgenet`，用于 Linux 开发/CLI 验证。当前项目的 VPN Bridge 和平台模块仍以 Windows 为中心，不能把这个命令当作“已完成 Ubuntu 客户端发布”的证明。

## 11. Windows 验证与发布收集

将 `edgenet.exe` 和它实际依赖的 DLL 放入干净目录，在目标 Windows 机器的 CMD 中执行：

```cmd
edgenet.exe version
edgenet.exe status
edgenet.exe doctor
```

不要用包含开发机遗留 DLL 的目录判断“单 EXE”是否成功。应在干净 Windows 测试机或全新目录测试，才能识别漏带的动态库。

至少验证：

```text
1. edgenet.exe 能启动
2. version/status/doctor 都有可读输出
3. Windows 网络探测能返回服务器路由、本地 IP、接口和公网 IP
4. TAP 驱动资源可被 build.rs 正确嵌入
5. VPN 连接测试仅使用独立测试证书，不使用生产凭据
```

## 12. 常见故障与处理

| 现象 | 原因 | 处理 |
| --- | --- | --- |
| 产物没有 `.exe` | 未指定 Windows target | 使用 `cargo build --release --target x86_64-pc-windows-gnu` |
| Windows 无法运行 EXE | DLL 缺失、SmartScreen、目标系统不兼容或双击后立即退出 | CMD 中运行；用 `objdump -p` 检查 DLL；核对 Windows 版本 |
| `reqwest` feature 不存在 | 旧示例用了 `rustls-tls` | 当前 `reqwest 0.13.5` 使用 `features = ["blocking", "rustls"]` |
| `reqwest` 未解析 | Cargo.toml 未加入依赖 | 不手填 `...` 版本占位符；以当前 Cargo.toml/lockfile 为准 |
| `include_bytes!` 找不到证书或 key | 敏感开发资产没有安全地放到 `vpn/` | 使用独立测试凭据补齐四个文件，绝不提交 |
| `libedgenet_vpn.a` 找不到 | C++ Bridge 未构建 | 先完成第 7 节，确认静态库路径 |
| CMake 找不到 OpenSSL/asio/fmt 等 | vcpkg triplet 或 toolchain 未传入 | 检查 `VCPKG_ROOT`、toolchain file 和 `x64-mingw-static` |
| Bridge include 路径缺失 | OpenVPN3 dynamic build 未完成 | 执行第 6 节；这是当前配置的已知耦合 |
| OpenVPN3 工作区有未提交修改 | 本地调试残留 | 不把工作区直接提交；将确认需要的修复整理为可审查补丁 |

## 13. 可复现发布前检查清单

```text
[ ] git status 干净，仅从提交的主仓库开始
[ ] Rust target 与 MinGW-w64 已安装
[ ] OpenVPN3 固定在 c33d2c44
[ ] vcpkg 固定在 a1cae005
[ ] OpenVPN3 x64 MinGW build 完成
[ ] vcpkg x64-mingw-static 依赖已安装
[ ] libedgenet_vpn.a 已生成
[ ] TAP 驱动输入文件齐全
[ ] VPN 测试资产来自安全渠道，未被 git 跟踪
[ ] cargo build --release --target x86_64-pc-windows-gnu 成功
[ ] file 显示 PE32+ Windows x64
[ ] 在干净 Windows 目录以 CMD 验证 version/status/doctor
[ ] 未提交私钥、证书、token、target、vcpkg 安装目录或构建缓存
```

## 14. 后续应改进的构建工程化项

当前步骤能描述现有构建，但仍有明确的改进方向：

1. 将 OpenVPN3 与 vcpkg 固定为 Git submodule 或以锁定版本的拉取脚本管理。
2. 把本机 OpenVPN3 调试改动整理成受审查的补丁或删除。
3. 统一 Bridge 的 static/dynamic triplet，去掉硬编码 include 路径。
4. 让 CMake 负责生成 Bridge，避免要求开发者手工记住多个构建目录。
5. 为敏感测试资产提供安全的注入方式及公开的无秘密样例。
6. 添加 CI：至少验证 Rust 格式、Windows 交叉编译和发布目录依赖完整性。
7. Linux 平台实现完成后，将 Linux 构建与 Windows 发布构建拆为两个明确的 CI job。

在这些工程化改进完成前，构建人员应严格按本文顺序执行，并把构建所用的 Rust、MinGW、OpenVPN3、vcpkg 版本记录在发布说明中。
