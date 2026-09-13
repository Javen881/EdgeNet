pub const DEFAULT_SERVER: &str = "javen01.freeddns.org";
pub const DEFAULT_PORT: u16 = 51280;
pub const VPN_SERVER_HOST: &str = DEFAULT_SERVER;
pub const VPN_SERVER_PORT: u16 = DEFAULT_PORT;
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


fn bytes_to_text(
    data: &'static [u8],
    name: &str,
) -> Result<&'static str, String> {
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
    if !all_assets_ready() {
        return Err(
            "embedded VPN assets are incomplete"
                .to_string(),
        );
    }

    let server = server.trim();

    let server = if server.is_empty() {
        DEFAULT_SERVER
    } else {
        server
    };

    let port = if port == 0 {
        DEFAULT_PORT
    } else {
        port
    };


    let ca = bytes_to_text(
        CA_CERT,
        "CA certificate",
    )?;

    let cert = bytes_to_text(
        CLIENT_CERT,
        "client certificate",
    )?;

    let key = bytes_to_text(
        CLIENT_KEY,
        "client private key",
    )?;

    let ta = bytes_to_text(
        TLS_AUTH_KEY,
        "TLS auth key",
    )?;


    let config = format!(
r#"client
dev tun

proto tcp-client
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
