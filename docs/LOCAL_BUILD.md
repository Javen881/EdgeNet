# 本次 Windows x64 构建

当前机器不需要 sudo，工具链位于仓库外的 `../build-tools/`。
新终端先执行（路径仅针对当前机器）：

```bash
source /home/javen03/data/VPN/build-tools/env.sh
cd /home/javen03/data/VPN/EdgeNet
```

依赖源码版本沿用 `BUILDING.md` 中的 OpenVPN3 `c33d2c44` 和 vcpkg `a1cae005`。
本次使用 Rust 1.99.0、MinGW GCC 13-posix，C++ 与 Rust 都使用 Release。

## 静态依赖

```bash
native/vcpkg/vcpkg install asio fmt jsoncpp lz4 openssl tap-windows6 xxhash \
  --triplet x64-mingw-static \
  --overlay-ports=native/openvpn3/deps/vcpkg-ports --disable-metrics
```

CMake 已改为从当前 triplet 查找 `tap-windows.h`，不再需要预先构建
OpenVPN3 的 `x64-mingw-dynamic` 目录。`build.rs` 显式链接桥接层所需的静态库，
并在桥接归档变更时重新链接 Rust 程序。

## 编译

```bash
cmake -S native/bridge -B native/bridge/build-static -G Ninja \
  -DCMAKE_SYSTEM_NAME=Windows -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_C_COMPILER=x86_64-w64-mingw32-gcc \
  -DCMAKE_CXX_COMPILER=x86_64-w64-mingw32-g++ \
  -DCMAKE_TOOLCHAIN_FILE="$PWD/native/vcpkg/scripts/buildsystems/vcpkg.cmake" \
  -DVCPKG_TARGET_TRIPLET=x64-mingw-static -DVCPKG_APPLOCAL_DEPS=OFF
cmake --build native/bridge/build-static --parallel 4
cargo build --release --locked --target x86_64-pc-windows-gnu --bins
```

GUI 产物是 `target/x86_64-pc-windows-gnu/release/edgenet_gui.exe`。
`edgenet.exe` 是命令行诊断程序，不是图形客户端。

## 测试证书

客户端编译输入在 `vpn/`，服务端部署证书在 `svpn/`；完整 CA 工作目录在
`../edgenet-test-pki/`。这些是本次 EasyRSA 生成的测试身份。客户端 EXE 内嵌客户端私钥；
不要把 EXE 当作不含凭据的通用公开发布文件。
服务端须使用匹配的 CA、服务端证书和 `ta.key`。CA 私钥不需要部署到服务器。

独立验证配置生成逻辑，无需链接 Windows Bridge：

```bash
rustc --edition 2024 --test src/vpn.rs -o ../build-tools/vpn-profile-tests
../build-tools/vpn-profile-tests
```

交叉编译和 PE/DLL 检查不能替代 Windows 上的启动、驱动安装、真实 VPN 连接和 iperf 测试。
性能复测见 `PERFORMANCE.md`。

## 本次验证结果

2026-10-13：四个 Windows x64 Release 程序均编译成功，配置测试 2/2 通过。
`objdump -p` 检查四个程序的导入表，未发现额外 MinGW/OpenSSL DLL 依赖。
第三方 C++ 代码有编译警告，Rust 的共享模块有未使用项警告，无编译或链接错误。
发布文件位于 `dist/windows/`，压缩包为 `dist/windows/EdgeNet-windows-x64-test.zip`。
本机没有 Windows 运行环境，尚未执行实际启动或隧道性能测试。
