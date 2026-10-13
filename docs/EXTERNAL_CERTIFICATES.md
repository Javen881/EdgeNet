# 外部证书

GUI 的 Certificates 区域默认选择 **Built-in (default)**，沿用程序内置身份。
切换到 **External files** 后，为下列四项分别点击 Browse，或粘贴文件路径：

- CA certificate：服务端所属 CA 的 PEM 证书（`.crt` / `.pem`）。
- Client certificate：该 CA 签发的客户端 PEM 证书。
- Client private key：与客户端证书匹配的未加密 PEM 私钥。
- TLS auth key (ta.key)：与服务端一致的 OpenVPN 静态认证密钥。

连接时读取这些文件并以内存 inline 配置交给 OpenVPN3，不复制到程序目录，
不记录私钥内容，也不将外部证书写入 EXE。重启程序默认回到内置证书；外部路径不保存。
连接期间不能切换证书来源。窗口内容可滚动。

选择外部文件后，缺失或错误文件会阻止连接，不会回退到内置证书或混用身份。
程序先检查文件大小、文本和 PEM 标记；实际证书、私钥和信任验证仍由 OpenVPN3 完成。

当前入口只替换证书材料，不导入完整 `.ovpn` 配置。协议、服务器和端口仍在 GUI 设置。
仍使用 `tls-auth`、客户端方向 1（服务端方向 0）。暂不支持加密私钥密码、PKCS#12、
`tls-crypt` 或无 `tls-auth` 的配置。不同服务端如果有其他认证设置，需要另行适配。

已通过 5 项配置/文件测试，并成功交叉编译 Windows x64 Release。
文件选择对话框和真实第三方服务端连接仍需在 Windows 上验证。
