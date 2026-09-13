use clap::{Parser, Subcommand};

mod platform;
mod vpn;
#[derive(Parser)]
#[command(
    name = "EdgeNet",
    version,
    about = "EdgeNet portable virtual network client"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Show EdgeNet status
    Status,

    /// Run diagnostics
    Doctor,

    /// Show version
    Version,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Status) => status(),
        Some(Commands::Doctor) => doctor(),
        Some(Commands::Version) => version(),
        None => status(),
    }
}

fn status() {
    println!("EdgeNet");
    println!("================================");
    println!("Version       : {}", env!("CARGO_PKG_VERSION"));
    println!("Status        : Offline");
    println!("Virtual IP    : -");

    #[cfg(target_os = "windows")]
    {
        use platform::windows::network;

        println!(
            "Server        : {}:{}",
            network::server_host(),
            network::server_port()
        );

        match network::get_server_routes() {
            Ok(routes) => {
                //
                // status 默认显示第一条可用路由。
                //
                // 如果 DNS 只有一个 A 记录，
                // 就只有这一条。
                //
                if let Some(route) = routes.first() {
                    println!("Resolved IP   : {}", route.server_ip);

                    println!("Local IP      : {}", route.local_ip);

                    println!("Interface     : {}", route.interface_name);
                }
            }

            Err(e) => {
                println!("Resolved IP   : Error");
                println!("Local IP      : Error");
                println!("Interface     : Error");
                println!("Route Error   : {}", e);
            }
        }

        match network::get_public_ipv4() {
            Ok(ip) => {
                println!("Public IP     : {}", ip);
            }

            Err(e) => {
                println!("Public IP     : Unknown");
                println!("Public Error  : {}", e);
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        println!("Server        : -");
        println!("Resolved IP   : -");
        println!("Local IP      : -");
        println!("Interface     : -");
        println!("Public IP     : -");
    }
}

fn doctor() {
    println!("EdgeNet Doctor");
    println!("================================");
    println!("Version       : {}", env!("CARGO_PKG_VERSION"));

    #[cfg(target_os = "windows")]
    {
        use platform::windows::network;

        println!("Platform      : Windows");

        println!(
            "Server        : {}:{}",
            network::server_host(),
            network::server_port()
        );

        println!();

        println!("DNS / Route");
        println!("--------------------------------");

        match network::get_server_routes() {
            Ok(routes) => {
                for (index, route) in routes.iter().enumerate() {
                    println!("Route {}", index + 1);

                    println!("  Server IP   : {}", route.server_ip);

                    println!("  Local IP    : {}", route.local_ip);

                    println!("  Interface   : {}", route.interface_name);
                }
            }

            Err(e) => {
                println!("Route Error   : {}", e);
            }
        }

        println!();

        println!("Public Network");
        println!("--------------------------------");

        match network::get_public_ipv4() {
            Ok(ip) => {
                println!("Public IP     : {}", ip);
            }

            Err(e) => {
                println!("Public IP     : Unknown");
                println!("Error         : {}", e);
            }
        }

        println!();

        println!("VPN Assets");
        println!("--------------------------------");

        let assets = vpn::check_embedded_assets();

        println!(
            "CA Certificate : {}",
            if assets.ca_cert { "OK" } else { "ERROR" }
        );

        println!(
            "Client Cert    : {}",
            if assets.client_cert { "OK" } else { "ERROR" }
        );

        println!(
            "Client Key     : {}",
            if assets.client_key { "OK" } else { "ERROR" }
        );

        println!(
            "TLS Auth Key   : {}",
            if assets.tls_auth_key { "OK" } else { "ERROR" }
        );

        println!(
            "VPN Assets     : {}",
            if vpn::all_assets_ready() {
                "Ready"
            } else {
                "Incomplete"
            }
        );

        println!("Tunnel");
        println!("--------------------------------");
        println!("Status        : Not configured");
    }

    #[cfg(target_os = "linux")]
    {
        println!("Platform      : Linux");
        println!("Tunnel        : Not configured");
    }
}

fn version() {
    println!("EdgeNet {}", env!("CARGO_PKG_VERSION"));
}
