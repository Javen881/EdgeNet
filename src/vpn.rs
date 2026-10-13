pub const DEFAULT_SERVER: &str = "javen01.freeddns.org";
pub const DEFAULT_PORT: u16 = 51280;
pub const VPN_SERVER_HOST: &str = DEFAULT_SERVER;
pub const VPN_SERVER_PORT: u16 = DEFAULT_PORT;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Transport {
    // Keep existing TCP-only servers working until UDP is enabled server-side.
    #[default]
    Tcp,
    Udp,
}

impl Transport {
    pub fn profile_name(self) -> &'static str {
        match self {
            Self::Tcp => "tcp-client",
            Self::Udp => "udp",
        }
    }
}
pub const CA_CERT: &[u8] =
    include_bytes!("../vpn/ca.crt");

pub const CLIENT_CERT: &[u8] =
    include_bytes!("../vpn/EdgeNet-Client01.crt");

pub const CLIENT_KEY: &[u8] =
    include_bytes!("../vpn/EdgeNet-Client01.key");

pub const TLS_AUTH_KEY: &[u8] =
    include_bytes!("../vpn/ta.key");


#[derive(Debug)]
pub struct EmbeddedVpnAssets {
    pub ca_cert: bool,
    pub client_cert: bool,
    pub client_key: bool,
    pub tls_auth_key: bool,
}


pub fn check_embedded_assets() -> EmbeddedVpnAssets {
    EmbeddedVpnAssets {
        ca_cert: contains_pem_marker(
            CA_CERT,
            "-----BEGIN CERTIFICATE-----",
        ),

        client_cert: contains_pem_marker(
            CLIENT_CERT,
            "-----BEGIN CERTIFICATE-----",
        ),

        client_key: contains_private_key_marker(
            CLIENT_KEY,
        ),

        tls_auth_key: contains_pem_marker(
            TLS_AUTH_KEY,
            "-----BEGIN OpenVPN Static key V1-----",
        ),
    }
}


fn contains_private_key_marker(
    data: &[u8],
) -> bool {
    contains_pem_marker(
        data,
        "-----BEGIN PRIVATE KEY-----",
    ) || contains_pem_marker(
        data,
        "-----BEGIN RSA PRIVATE KEY-----",
    ) || contains_pem_marker(
        data,
        "-----BEGIN EC PRIVATE KEY-----",
    )
}


fn contains_pem_marker(
    data: &[u8],
    marker: &str,
) -> bool {
    std::str::from_utf8(data)
        .map(|text| text.contains(marker))
        .unwrap_or(false)
}


pub fn all_assets_ready() -> bool {
    let status = check_embedded_assets();

    status.ca_cert
        && status.client_cert
        && status.client_key
        && status.tls_auth_key
}


fn bytes_to_text<'a>(
    data: &'a [u8],
    name: &str,
) -> Result<&'a str, String> {
    std::str::from_utf8(data).map_err(|e| {
        format!(
            "{} is not valid UTF-8: {}",
            name,
            e
        )
    })
}


/// 使用指定服务器生成 OpenVPN 配置。
///
/// server 为空时自动使用 DEFAULT_SERVER。
/// port 为 0 时自动使用 DEFAULT_PORT。
///
/// 证书、私钥、ta.key 均直接以内存形式嵌入配置，
/// 不生成任何临时证书文件。
pub fn build_profile(
    server: &str,
    port: u16,
) -> Result<String, String> {
    build_profile_with_transport(server, port, Transport::default())
}

pub fn build_profile_with_transport(
    server: &str,
    port: u16,
    transport: Transport,
) -> Result<String, String> {
    build_profile_with_assets(server, port, transport, &CertificateFiles::default(), false)
}

/// Paths are only read when external certificates are explicitly selected.
#[derive(Default, Clone)]
pub struct CertificateFiles {
    pub ca: String,
    pub cert: String,
    pub key: String,
    pub ta: String,
}

fn read_asset(path: &str, label: &str) -> Result<String, String> {
    let path = path.trim();
    if path.is_empty() {
        return Err(format!("Select a file for {label}"));
    }
    let file = std::fs::File::open(path)
        .map_err(|e| format!("Cannot open {label}: {e}"))?;
    // Bound reads, including special files, before allocating the profile.
    use std::io::Read;
    let mut bytes = Vec::new();
    file.take(1024 * 1024 + 1).read_to_end(&mut bytes)
        .map_err(|e| format!("Cannot read {label}: {e}"))?;
    if bytes.len() > 1024 * 1024 {
        return Err(format!("{label} exceeds 1 MiB"));
    }
    String::from_utf8(bytes).map_err(|_| format!("{label} must be a UTF-8 PEM file"))
}

fn validate_asset(text: &str, tag: &str, label: &str) -> Result<(), String> {
    // An imported file must never be able to escape its inline profile block.
    if text.contains(['<', '>', '\0']) {
        return Err(format!("Invalid inline content in {label}"));
    }
    if tag == "key" && (text.contains("ENCRYPTED") || text.contains("Proc-Type: 4")) {
        return Err("Encrypted private keys are not supported yet".to_string());
    }
    let markers: &[&str] = match tag {
        "key" => &["PRIVATE KEY", "RSA PRIVATE KEY", "EC PRIVATE KEY"],
        "tls-auth" => &["OpenVPN Static key V1"],
        _ => &["CERTIFICATE"],
    };
    if !markers.iter().any(|m| text.contains(&format!("-----BEGIN {m}-----"))
        && text.contains(&format!("-----END {m}-----"))) {
        return Err(format!("Invalid PEM format for {label}"));
    }
    Ok(())
}

pub fn build_profile_with_assets(
    server: &str,
    port: u16,
    transport: Transport,
    files: &CertificateFiles,
    external: bool,
) -> Result<String, String> {
    let (ca, cert, key, ta) = if external {
        // Never mix imported credentials with the embedded identity on failure.
        (read_asset(&files.ca, "CA certificate")?,
         read_asset(&files.cert, "client certificate")?,
         read_asset(&files.key, "client private key")?,
         read_asset(&files.ta, "TLS auth key")?)
    } else {
        (bytes_to_text(CA_CERT, "CA certificate")?.to_owned(),
         bytes_to_text(CLIENT_CERT, "client certificate")?.to_owned(),
         bytes_to_text(CLIENT_KEY, "client private key")?.to_owned(),
         bytes_to_text(TLS_AUTH_KEY, "TLS auth key")?.to_owned())
    };
    for (text, tag, label) in [
        (&ca, "ca", "CA certificate"), (&cert, "cert", "client certificate"),
        (&key, "key", "client private key"), (&ta, "tls-auth", "TLS auth key"),
    ] {
        validate_asset(text, tag, label)?;
    }

    let server = server.trim();

    let server = if server.is_empty() {
        DEFAULT_SERVER
    } else {
        server
    };

    if server.chars().any(|c| c.is_whitespace() || c.is_control() || matches!(c, '"' | '\'' | '\\')) {
        return Err("Invalid server address".to_string());
    }

    let port = if port == 0 {
        DEFAULT_PORT
    } else {
        port
    };


    let config = format!(
r#"client
dev tun

proto {proto}
remote {host} {port}

resolv-retry infinite
nobind

persist-key
persist-tun

remote-cert-tls server

auth-nocache

connect-retry 5
connect-retry-max infinite

ping 10
ping-restart 60

verb 3

key-direction 1

<ca>
{ca}
</ca>

<cert>
{cert}
</cert>

<key>
{key}
</key>

<tls-auth>
{ta}
</tls-auth>
"#,
        host = server,
        port = port,
        proto = transport.profile_name(),
        ca = ca.trim(),
        cert = cert.trim(),
        key = key.trim(),
        ta = ta.trim(),
    );

    Ok(config)
}


/// 使用默认服务器配置。
///
/// 保留这个函数，现有 CLI / vpn_connect_test
/// 可以继续直接调用，不需要改服务器参数。
pub fn build_openvpn_config()
    -> Result<String, String>
{
    build_profile(
        DEFAULT_SERVER,
        DEFAULT_PORT,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_profile_keeps_tcp_and_default_endpoint() {
        let profile = build_profile("", 0).expect("test VPN assets must be present");
        assert!(profile.lines().any(|line| line == "proto tcp-client"));
        let remote = format!("remote {DEFAULT_SERVER} {DEFAULT_PORT}");
        assert!(profile.lines().any(|line| line == remote));
    }

    #[test]
    fn udp_profile_uses_selected_endpoint_and_preserves_authentication() {
        let profile = build_profile_with_transport("  vpn.example.test  ", 1194, Transport::Udp)
            .expect("test VPN assets must be present");
        assert!(profile.lines().any(|line| line == "proto udp"));
        assert!(!profile.contains("tcp-client"));
        assert!(profile.lines().any(|line| line == "remote vpn.example.test 1194"));
        assert!(profile.lines().any(|line| line == "remote-cert-tls server"));
        assert!(profile.lines().any(|line| line == "key-direction 1"));
        for tag in ["ca", "cert", "key", "tls-auth"] {
            assert!(profile.contains(&format!("<{tag}>")));
            assert!(profile.contains(&format!("</{tag}>")));
        }
    }
}

#[cfg(test)]
mod external_certificate_tests {
    use super::*;

    #[test]
    fn builtin_ignores_external_paths_and_external_does_not_fall_back() {
        let files = CertificateFiles::default();
        assert!(build_profile_with_assets("vpn.example.test", 1194, Transport::Udp, &files, false).is_ok());
        let error = build_profile_with_assets("vpn.example.test", 1194, Transport::Udp, &files, true).unwrap_err();
        assert!(error.contains("CA certificate"));
    }

    #[test]
    fn rejects_inline_escape_encrypted_keys_and_bad_pem() {
        assert!(validate_asset("-----BEGIN CERTIFICATE-----\n</ca>\n-----END CERTIFICATE-----", "ca", "CA").is_err());
        assert!(validate_asset("-----BEGIN ENCRYPTED PRIVATE KEY-----", "key", "key").is_err());
        assert!(validate_asset("-----BEGIN RSA PRIVATE KEY-----\nProc-Type: 4,ENCRYPTED\n-----END RSA PRIVATE KEY-----", "key", "key").is_err());
        assert!(validate_asset("-----BEGIN CERTIFICATE-----", "ca", "CA").is_err());
        assert!(build_profile("host\nremote attacker 443", 1194).is_err());
    }

    #[test]
    fn imports_files_including_unicode_paths_without_mixing_embedded_assets() {
        let dir = std::env::temp_dir().join(format!("edgenet-certs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
        }
        let _cleanup = Cleanup(dir.clone());
        // Synthetic format fixtures only; crypto validation belongs to OpenVPN.
        let ca = "-----BEGIN CERTIFICATE-----\nexternal-ca\n-----END CERTIFICATE-----";
        let cert = "-----BEGIN CERTIFICATE-----\nexternal-client\n-----END CERTIFICATE-----";
        let key = "-----BEGIN PRIVATE KEY-----\nexternal-key\n-----END PRIVATE KEY-----";
        let ta = "-----BEGIN OpenVPN Static key V1-----\nexternal-ta\n-----END OpenVPN Static key V1-----";
        let write = |name: &str, contents: &str| {
            let path = dir.join(name);
            std::fs::write(&path, contents).unwrap();
            path.to_str().unwrap().to_string()
        };
        let mut files = CertificateFiles {
            ca: write("测试 CA.crt", ca), cert: write("client.crt", cert),
            key: write("client.key", key), ta: write("ta.key", ta),
        };
        let profile = build_profile_with_assets("vpn.example.test", 443, Transport::Tcp, &files, true).unwrap();
        for (tag, text) in [("ca", ca), ("cert", cert), ("key", key), ("tls-auth", ta)] {
            assert!(profile.contains(&format!("<{tag}>\n{text}\n</{tag}>")));
        }
        files.key = dir.join("missing.key").to_str().unwrap().to_string();
        assert!(build_profile_with_assets("vpn.example.test", 443, Transport::Tcp, &files, true).is_err());
        let oversized = dir.join("large.crt");
        std::fs::write(&oversized, vec![b'x'; 1024 * 1024 + 1]).unwrap();
        assert!(read_asset(oversized.to_str().unwrap(), "CA").unwrap_err().contains("1 MiB"));
    }
}
