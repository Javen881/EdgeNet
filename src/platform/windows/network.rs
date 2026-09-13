use std::net::{IpAddr, Ipv4Addr, SocketAddrV4, ToSocketAddrs, UdpSocket};
use std::time::Duration;

const SERVER_HOST: &str = "javen01.freeddns.org";
const SERVER_PORT: u16 = 51280;

#[derive(Debug, Clone)]
pub struct RouteInfo {
    pub server_ip: Ipv4Addr,
    pub local_ip: Ipv4Addr,
    pub interface_name: String,
}

/// EdgeNet Server 域名
pub fn server_host() -> &'static str {
    SERVER_HOST
}

/// EdgeNet Server 端口
pub fn server_port() -> u16 {
    SERVER_PORT
}

/// DNS 解析 EdgeNet Server。
///
/// 支持同一个域名返回多个 IPv4 地址。
pub fn resolve_server_ipv4() -> Result<Vec<Ipv4Addr>, String> {
    let addrs = (SERVER_HOST, SERVER_PORT)
        .to_socket_addrs()
        .map_err(|e| format!("DNS resolve failed: {e}"))?;

    let mut result = Vec::new();

    for addr in addrs {
        if let IpAddr::V4(ip) = addr.ip() {
            if !result.contains(&ip) {
                result.push(ip);
            }
        }
    }

    if result.is_empty() {
        return Err(format!("no IPv4 address found for {}", SERVER_HOST));
    }

    Ok(result)
}

/// 判断 Windows 去指定目标 IPv4 时会选择哪个本地源 IP。
///
/// UDP connect 不会发送实际业务数据。
/// 它主要让操作系统根据自己的路由表选择出口。
///
/// 可以处理：
/// - 多网卡
/// - 多个内网 IP
/// - 多默认路由
/// - 静态路由
/// - VPN 路由
pub fn get_route_local_ipv4(destination: Ipv4Addr) -> Result<Ipv4Addr, String> {
    let socket = UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("bind failed: {e}"))?;

    let target = SocketAddrV4::new(destination, SERVER_PORT);

    socket
        .connect(target)
        .map_err(|e| format!("route select failed: {e}"))?;

    let local = socket
        .local_addr()
        .map_err(|e| format!("local_addr failed: {e}"))?;

    match local.ip() {
        IpAddr::V4(ip) => Ok(ip),
        IpAddr::V6(_) => Err("Windows selected IPv6 unexpectedly".to_string()),
    }
}

/// 查询当前 EdgeNet Server 各个 A 记录对应的出口。
pub fn get_server_routes() -> Result<Vec<RouteInfo>, String> {
    let server_ips = resolve_server_ipv4()?;

    let mut routes = Vec::new();
    let mut errors = Vec::new();

    for server_ip in server_ips {
        match get_route_local_ipv4(server_ip) {
            Ok(local_ip) => {
                let interface_name =
                    get_interface_name_by_ip(local_ip).unwrap_or_else(|_| "Unknown".to_string());

                routes.push(RouteInfo {
                    server_ip,
                    local_ip,
                    interface_name,
                });
            }

            Err(e) => {
                errors.push(format!("{server_ip}: {e}"));
            }
        }
    }

    if routes.is_empty() {
        return Err(format!(
            "no usable route to EdgeNet server: {}",
            errors.join(" | ")
        ));
    }

    Ok(routes)
}

/// 获取公网 IPv4。
///
/// 顺序容灾：
///
/// 1. ipify
/// 2. AWS
/// 3. icanhazip
///
/// 第一个成功就返回。
pub fn get_public_ipv4() -> Result<String, String> {
    const IP_SERVICES: &[&str] = &[
        "https://api.ipify.org",
        "https://checkip.amazonaws.com",
        "https://icanhazip.com",
    ];

    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(3))
        .user_agent("EdgeNet/0.1")
        .build()
        .map_err(|e| format!("HTTP client failed: {e}"))?;

    let mut errors = Vec::new();

    for url in IP_SERVICES {
        match query_public_ipv4(&client, url) {
            Ok(ip) => return Ok(ip.to_string()),

            Err(e) => {
                errors.push(format!("{url}: {e}"));
            }
        }
    }

    Err(format!(
        "all public IP services failed: {}",
        errors.join(" | ")
    ))
}

fn query_public_ipv4(client: &reqwest::blocking::Client, url: &str) -> Result<Ipv4Addr, String> {
    let response = client
        .get(url)
        .send()
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;

    let text = response.text().map_err(|e| e.to_string())?;

    text.trim()
        .parse::<Ipv4Addr>()
        .map_err(|_| format!("invalid IPv4 response: {:?}", text.trim()))
}

/// 根据本地 IPv4 找 Windows 网卡 FriendlyName。
#[cfg(target_os = "windows")]
pub fn get_interface_name_by_ip(target_ip: Ipv4Addr) -> Result<String, String> {
    use std::ptr::null_mut;

    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetAdaptersAddresses, IP_ADAPTER_ADDRESSES_LH,
    };

    use windows_sys::Win32::Networking::WinSock::{AF_INET, SOCKADDR_IN};

    const GAA_FLAG_INCLUDE_PREFIX: u32 = 0x0010;
    const ERROR_BUFFER_OVERFLOW: u32 = 111;

    let mut size: u32 = 15 * 1024;

    let mut buffer = vec![0u8; size as usize];

    let mut result = unsafe {
        GetAdaptersAddresses(
            AF_INET as u32,
            GAA_FLAG_INCLUDE_PREFIX,
            null_mut(),
            buffer.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH,
            &mut size,
        )
    };

    if result == ERROR_BUFFER_OVERFLOW {
        buffer.resize(size as usize, 0);

        result = unsafe {
            GetAdaptersAddresses(
                AF_INET as u32,
                GAA_FLAG_INCLUDE_PREFIX,
                null_mut(),
                buffer.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH,
                &mut size,
            )
        };
    }

    if result != 0 {
        return Err(format!("GetAdaptersAddresses failed: {result}"));
    }

    let mut adapter = buffer.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;

    while !adapter.is_null() {
        let adapter_ref = unsafe { &*adapter };

        let mut unicast = adapter_ref.FirstUnicastAddress;

        while !unicast.is_null() {
            let unicast_ref = unsafe { &*unicast };

            let sockaddr_ptr = unicast_ref.Address.lpSockaddr;

            if !sockaddr_ptr.is_null() {
                let sockaddr_in = unsafe { &*(sockaddr_ptr as *const SOCKADDR_IN) };

                if sockaddr_in.sin_family == AF_INET {
                    let raw = unsafe { sockaddr_in.sin_addr.S_un.S_addr };

                    let ip = Ipv4Addr::from(raw.to_ne_bytes());

                    if ip == target_ip {
                        if adapter_ref.FriendlyName.is_null() {
                            return Err("adapter friendly name is null".to_string());
                        }

                        let mut len = 0usize;

                        unsafe {
                            while *adapter_ref.FriendlyName.add(len) != 0 {
                                len += 1;
                            }
                        }

                        let slice =
                            unsafe { std::slice::from_raw_parts(adapter_ref.FriendlyName, len) };

                        let name = String::from_utf16(slice)
                            .map_err(|e| format!("UTF16 decode failed: {e}"))?;

                        return Ok(name);
                    }
                }
            }

            unicast = unsafe { (*unicast).Next };
        }

        adapter = adapter_ref.Next;
    }

    Err(format!("interface not found for {}", target_ip))
}
