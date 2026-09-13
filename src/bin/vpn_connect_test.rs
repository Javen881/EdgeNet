#[path = "../native_vpn.rs"]
mod native_vpn;

#[path = "../vpn.rs"]
mod vpn;

use std::thread;
use std::time::Duration;

fn main() {
    println!("EdgeNet OpenVPN3 Connect Test");
    println!("============================");

    println!(
        "Server       : {}:{}",
        vpn::VPN_SERVER_HOST,
        vpn::VPN_SERVER_PORT
    );

    println!("Protocol     : TCP");
    println!("Tunnel       : TUN");

    println!();
    println!("[1] Checking embedded VPN assets...");

    let assets = vpn::check_embedded_assets();

    println!("CA Cert      : {}", yes_no(assets.ca_cert));
    println!("Client Cert  : {}", yes_no(assets.client_cert));
    println!("Client Key   : {}", yes_no(assets.client_key));
    println!("TLS Auth Key : {}", yes_no(assets.tls_auth_key));

    if !vpn::all_assets_ready() {
        eprintln!();
        eprintln!("ERROR: Embedded VPN assets are incomplete.");
        std::process::exit(1);
    }

    println!();
    println!("[2] Building OpenVPN configuration...");

    let config = match vpn::build_openvpn_config() {
        Ok(config) => config,

        Err(error) => {
            eprintln!("ERROR: {}", error);
            std::process::exit(1);
        }
    };

    println!("Config       : Ready");
    println!("Config Size  : {} bytes", config.len());

    println!();
    println!("[3] Starting OpenVPN3...");

    if let Err(error) = native_vpn::connect(&config) {
        eprintln!("ERROR: {}", error);
        std::process::exit(1);
    }

    println!("Connect call : OK");

    println!();
    println!("[4] Waiting for VPN connection...");
    println!("Press Ctrl+C to terminate.");
    println!();

    let mut last_state: Option<native_vpn::VpnState> = None;

    loop {
        let state = native_vpn::status();

        if last_state != Some(state) {
            println!("VPN State    : {:?}", state);
            last_state = Some(state);
        }

        match state {
            native_vpn::VpnState::Connected => {
                let ip = native_vpn::virtual_ip();

                println!(
                    "Virtual IP   : {}",
                    if ip.is_empty() {
                        "(not reported)"
                    } else {
                        &ip
                    }
                );

                println!();
                println!("================================");
                println!("EdgeNet VPN connected.");
                println!("================================");
                println!();
                println!("Process must remain running.");
                println!("Press Ctrl+C to disconnect.");

                connected_loop();
            }

            native_vpn::VpnState::Error => {
                let error = native_vpn::last_error();

                eprintln!();

                if error.is_empty() {
                    eprintln!("VPN ERROR: Unknown OpenVPN3 error.");
                } else {
                    eprintln!("VPN ERROR: {}", error);
                }

                let _ = native_vpn::disconnect();

                std::process::exit(2);
            }

            native_vpn::VpnState::Disconnected => {
                let error = native_vpn::last_error();

                if !error.is_empty() {
                    eprintln!("VPN disconnected: {}", error);
                    std::process::exit(3);
                }
            }

            native_vpn::VpnState::Connecting
            | native_vpn::VpnState::Disconnecting
            | native_vpn::VpnState::Unknown(_) => {}
        }

        thread::sleep(Duration::from_secs(1));
    }
}

fn connected_loop() -> ! {
    loop {
        let state = native_vpn::status();

        match state {
            native_vpn::VpnState::Connected => {
                thread::sleep(Duration::from_secs(2));
            }

            native_vpn::VpnState::Error => {
                let error = native_vpn::last_error();

                if error.is_empty() {
                    eprintln!("VPN ERROR.");
                } else {
                    eprintln!("VPN ERROR: {}", error);
                }

                std::process::exit(4);
            }

            native_vpn::VpnState::Disconnected => {
                println!("VPN disconnected.");
                std::process::exit(0);
            }

            other => {
                println!("VPN State    : {:?}", other);
                thread::sleep(Duration::from_secs(1));
            }
        }
    }
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "OK"
    } else {
        "FAILED"
    }
}
