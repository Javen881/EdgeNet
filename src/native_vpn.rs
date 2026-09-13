use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};

#[link(name = "edgenet_vpn",kind = "static")]
unsafe extern "C" {
    fn edgenet_vpn_connect(
        config: *const c_char,
    ) -> c_int;

    fn edgenet_vpn_disconnect() -> c_int;

    fn edgenet_vpn_status() -> c_int;

    fn edgenet_vpn_last_error()
        -> *const c_char;

    fn edgenet_vpn_virtual_ip()
        -> *const c_char;
    fn edgenet_vpn_gateway()
        -> *const c_char;

    fn edgenet_vpn_protocol()
        -> *const c_char;

    fn edgenet_vpn_tun_name()
        -> *const c_char;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VpnState {
    Error,
    Disconnected,
    Connecting,
    Connected,
    Disconnecting,
    Unknown(i32),
}

impl From<i32> for VpnState {
    fn from(value: i32) -> Self {
        match value {
            -1 => Self::Error,
            0 => Self::Disconnected,
            1 => Self::Connecting,
            2 => Self::Connected,
            3 => Self::Disconnecting,
            value => Self::Unknown(value),
        }
    }
}

pub fn connect(
    config: &str,
) -> Result<(), String> {
    let config = CString::new(config)
        .map_err(|_| {
            "OpenVPN config contains NUL byte"
                .to_string()
        })?;

    let result = unsafe {
        edgenet_vpn_connect(
            config.as_ptr(),
        )
    };

    if result == 0 {
        Ok(())
    } else {
        let error = last_error();

        if error.is_empty() {
            Err(format!(
                "VPN connect failed: {}",
                result
            ))
        } else {
            Err(error)
        }
    }
}

pub fn disconnect() -> Result<(), String> {
    let result = unsafe {
        edgenet_vpn_disconnect()
    };

    if result == 0 {
        Ok(())
    } else {
        let error = last_error();

        if error.is_empty() {
            Err(format!(
                "VPN disconnect failed: {}",
                result
            ))
        } else {
            Err(error)
        }
    }
}

pub fn status() -> VpnState {
    let result = unsafe {
        edgenet_vpn_status()
    };

    VpnState::from(result)
}

pub fn virtual_ip() -> String {
    unsafe {
        ptr_to_string(
            edgenet_vpn_virtual_ip(),
        )
    }
}

pub fn last_error() -> String {
    unsafe {
        ptr_to_string(
            edgenet_vpn_last_error(),
        )
    }
}

unsafe fn ptr_to_string(
    ptr: *const c_char,
) -> String {
    if ptr.is_null() {
        return String::new();
    }

    unsafe {
        CStr::from_ptr(ptr)
    }
    .to_string_lossy()
    .into_owned()
}
pub fn gateway() -> String {
    unsafe {
        ptr_to_string(
            edgenet_vpn_gateway(),
        )
    }
}

pub fn protocol() -> String {
    unsafe {
        ptr_to_string(
            edgenet_vpn_protocol(),
        )
    }
}

pub fn tun_name() -> String {
    unsafe {
        ptr_to_string(
            edgenet_vpn_tun_name(),
        )
    }
}
