#[path = "../native_vpn.rs"]
mod native_vpn;

fn main() {
    println!("EdgeNet OpenVPN3 FFI Test");
    println!("=========================");

    println!(
        "VPN State   : {:?}",
        native_vpn::status()
    );

    let virtual_ip =
        native_vpn::virtual_ip();

    println!(
        "Virtual IP  : {}",
        if virtual_ip.is_empty() {
            "-"
        } else {
            &virtual_ip
        }
    );

    let error =
        native_vpn::last_error();

    println!(
        "Last Error  : {}",
        if error.is_empty() {
            "-"
        } else {
            &error
        }
    );
}
