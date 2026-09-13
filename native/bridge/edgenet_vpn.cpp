#include "edgenet_vpn.hpp"

#include <client/ovpncli.hpp>
#include "wintun_manager.hpp"
#include <atomic>
#include <cstdio>
#include <memory>
#include <mutex>
#include <string>
#include <thread>
#include <utility>
#include <windows.h>
#include "tap_manager.hpp"
namespace
{

std::mutex g_mutex;

std::shared_ptr<openvpn::ClientAPI::OpenVPNClient> g_client;

std::thread g_worker;

std::atomic<int> g_state{
    EDGENET_VPN_DISCONNECTED
};

std::string g_last_error;
std::string g_virtual_ip;
std::string g_gateway;
std::string g_protocol;
std::string g_tun_name;

void set_last_error(
    const std::string &message)
{
    std::lock_guard<std::mutex> lock(g_mutex);
    g_last_error = message;
}

void clear_last_error()
{
    std::lock_guard<std::mutex> lock(g_mutex);
    g_last_error.clear();
}

void set_virtual_ip(
    const std::string &ip)
{
    std::lock_guard<std::mutex> lock(g_mutex);
    g_virtual_ip = ip;
}

void clear_virtual_ip()
{
    std::lock_guard<std::mutex> lock(g_mutex);
    g_virtual_ip.clear();
}
void set_gateway(
    const std::string &gateway)
{
    std::lock_guard<std::mutex> lock(g_mutex);
    g_gateway = gateway;
}

void clear_gateway()
{
    std::lock_guard<std::mutex> lock(g_mutex);
    g_gateway.clear();
}

void set_protocol(
    const std::string &protocol)
{
    std::lock_guard<std::mutex> lock(g_mutex);
    g_protocol = protocol;
}

void clear_protocol()
{
    std::lock_guard<std::mutex> lock(g_mutex);
    g_protocol.clear();
}
void set_tun_name(
    const std::string &name)
{
    std::lock_guard<std::mutex> lock(g_mutex);
    g_tun_name = name;
}

void clear_tun_name()
{
    std::lock_guard<std::mutex> lock(g_mutex);
    g_tun_name.clear();
}
void print_log(
    const char *prefix,
    const std::string &text)
{
    std::fprintf(
        stdout,
        "%s%s",
        prefix,
        text.c_str());

    if (text.empty() ||
        text.back() != '\n')
    {
        std::fprintf(stdout, "\n");
    }

    std::fflush(stdout);
}

class EdgeNetOpenVPNClient final
    : public openvpn::ClientAPI::OpenVPNClient
{
public:
    bool socket_protect(
        openvpn_io::detail::socket_type,
        std::string,
        bool) override
    {
        return true;
    }

    void event(
        const openvpn::ClientAPI::Event &ev)
        override
    {
        std::string line;

        line += "[OpenVPN3 EVENT] ";
        line += ev.name;

        if (!ev.info.empty())
        {
            line += " | ";
            line += ev.info;
        }

        if (ev.error)
        {
            line += " | error=1";
        }

        if (ev.fatal)
        {
            line += " | fatal=1";
        }

        print_log("", line);

        if (ev.name == "CONNECTED")
		{
			const auto info =
				connection_info();

			set_virtual_ip(
				info.vpnIp4);

			set_gateway(
				info.gw4);

			set_protocol(
				info.serverProto);

			set_tun_name(
				info.tunName);

			g_state.store(
				EDGENET_VPN_CONNECTED);

			return;
		}

        if (ev.name == "DISCONNECTED" ||
            ev.name == "EXITING")
        {
            g_state.store(
                EDGENET_VPN_DISCONNECTED);

            return;
        }

        if (ev.error)
        {
            std::string message =
                ev.name;

            if (!ev.info.empty())
            {
                message += ": ";
                message += ev.info;
            }

            set_last_error(message);

            if (ev.fatal)
            {
                g_state.store(
                    EDGENET_VPN_ERROR);
            }
        }
    }

    void acc_event(
        const openvpn::ClientAPI::
            AppCustomControlMessageEvent &)
        override
    {
    }

    void log(
        const openvpn::ClientAPI::LogInfo &log)
        override
    {
        print_log(
            "[OpenVPN3] ",
            log.text);
    }

    void external_pki_cert_request(
        openvpn::ClientAPI::
            ExternalPKICertRequest &req)
        override
    {
        req.error = true;
        req.errorText =
            "External PKI is not used by EdgeNet";
    }

    void external_pki_sign_request(
        openvpn::ClientAPI::
            ExternalPKISignRequest &req)
        override
    {
        req.error = true;
        req.errorText =
            "External PKI is not used by EdgeNet";
    }

    bool pause_on_connection_timeout()
        override
    {
        return false;
    }
};

void join_worker_if_finished()
{
    if (g_worker.joinable() &&
        g_state.load() !=
            EDGENET_VPN_CONNECTING &&
        g_state.load() !=
            EDGENET_VPN_CONNECTED &&
        g_state.load() !=
            EDGENET_VPN_DISCONNECTING)
    {
        g_worker.join();
    }
}

void vpn_worker(
    std::string config_text,
    std::shared_ptr<EdgeNetOpenVPNClient> client)
{
    try
    {
        openvpn::ClientAPI::Config config;

        config.content =
            std::move(config_text);

        config.compressionMode = "no";

        /*
         * First keep DCO disabled.
         *
         * We currently want to diagnose the classic
         * TAP/TUN path clearly before switching backend.
         */
        config.dco = false;

        /*
         * Use TAP-Windows6 path for the current test.
         *
         * false = TAP-Windows6
         * true  = Wintun
         */
        config.wintun = false;

        const auto eval =
            client->eval_config(config);

        if (eval.error)
        {
            set_last_error(
                "CONFIG: " +
                eval.message);

            g_state.store(
                EDGENET_VPN_ERROR);

            return;
        }
		 

        std::fprintf(
            stdout,
            "[EdgeNet] OpenVPN profile evaluated\n");

        std::fprintf(
            stdout,
            "[EdgeNet] Remote: %s:%s %s\n",
            eval.remoteHost.c_str(),
            eval.remotePort.c_str(),
            eval.remoteProto.c_str());

        std::fprintf(
            stdout,
            "[EdgeNet] Windows driver: %s\n",
            eval.windowsDriver.empty()
                ? "(default)"
                : eval.windowsDriver.c_str());

        std::fflush(stdout);
/// Wintun 初始化
// OpenVPN3 开始真正连接
//
std::string tap_error;

if (!edgenet::ensure_tap_adapter(tap_error))
{
    std::fprintf(
        stderr,
        "[EdgeNet] TAP initialization failed: %s\n",
        tap_error.c_str()
    );

    return;
}

std::fprintf(
    stderr,
    "[EdgeNet] TAP initialization complete\n"
);
const auto status =
            client->connect();

        if (status.error)
        {
            std::string message =
                status.status;

            if (!status.message.empty())
            {
                if (!message.empty())
                {
                    message += ": ";
                }

                message +=
                    status.message;
            }

            if (message.empty())
            {
                message =
                    "OpenVPN3 connect failed";
            }

            set_last_error(message);

            g_state.store(
                EDGENET_VPN_ERROR);

            return;
        }

        /*
         * connect() returns only after disconnect.
         */
        if (g_state.load() !=
            EDGENET_VPN_ERROR)
        {
            g_state.store(
                EDGENET_VPN_DISCONNECTED);
        }
    }
    catch (const std::exception &e)
    {
        set_last_error(
            std::string(
                "OpenVPN3 exception: ") +
            e.what());

        g_state.store(
            EDGENET_VPN_ERROR);
    }
    catch (...)
    {
        set_last_error(
            "OpenVPN3 unknown exception");

        g_state.store(
            EDGENET_VPN_ERROR);
    }

    std::lock_guard<std::mutex> lock(g_mutex);

    if (g_client.get() ==
        client.get())
    {
        g_client.reset();
    }
}

} // namespace

extern "C"
{

int edgenet_vpn_connect(
    const char *config)
{
    if (config == nullptr ||
        config[0] == '\0')
    {
        set_last_error(
            "VPN config is empty");

        return -1;
    }

    join_worker_if_finished();

    const int state =
        g_state.load();

    if (state ==
            EDGENET_VPN_CONNECTING ||
        state ==
            EDGENET_VPN_CONNECTED ||
        state ==
            EDGENET_VPN_DISCONNECTING)
    {
        set_last_error(
            "VPN is already active");

        return -2;
    }

    if (g_worker.joinable())
    {
        g_worker.join();
    }

    clear_last_error();
    clear_virtual_ip();
	clear_gateway();
	clear_protocol();
	clear_tun_name();

    auto client =
        std::make_shared<
            EdgeNetOpenVPNClient>();

    {
        std::lock_guard<std::mutex> lock(
            g_mutex);

        g_client =
            client;
    }

    g_state.store(
        EDGENET_VPN_CONNECTING);

    try
    {
        g_worker =
            std::thread(
                vpn_worker,
                std::string(config),
                client);
    }
    catch (const std::exception &e)
    {
        {
            std::lock_guard<std::mutex> lock(
                g_mutex);

            g_client.reset();
        }

        g_state.store(
            EDGENET_VPN_ERROR);

        set_last_error(
            std::string(
                "Failed to start VPN thread: ") +
            e.what());

        return -3;
    }

    return 0;
}

int edgenet_vpn_disconnect(void)
{
    const int state =
        g_state.load();

    if (state ==
            EDGENET_VPN_DISCONNECTED)
    {
        join_worker_if_finished();
        return 0;
    }

    g_state.store(
        EDGENET_VPN_DISCONNECTING);

    std::shared_ptr<
        openvpn::ClientAPI::OpenVPNClient>
        client;

    {
        std::lock_guard<std::mutex> lock(
            g_mutex);

        client =
            g_client;
    }

    if (client)
    {
        try
        {
            client->stop();
        }
        catch (const std::exception &e)
        {
            set_last_error(
                std::string(
                    "VPN stop failed: ") +
                e.what());
        }
        catch (...)
        {
            set_last_error(
                "VPN stop failed");
        }
    }

    if (g_worker.joinable())
    {
        g_worker.join();
    }

    {
        std::lock_guard<std::mutex> lock(
            g_mutex);

        g_client.reset();
    }

    clear_virtual_ip();
	clear_gateway();
	clear_protocol();
	clear_tun_name();

    g_state.store(
        EDGENET_VPN_DISCONNECTED);

    return 0;
}

int edgenet_vpn_status(void)
{
    join_worker_if_finished();

    return g_state.load();
}

const char *
edgenet_vpn_last_error(void)
{
    static thread_local std::string result;

    std::lock_guard<std::mutex> lock(
        g_mutex);

    result =
        g_last_error;

    return result.c_str();
}

const char *
edgenet_vpn_virtual_ip(void)
{
    static thread_local std::string result;

    std::lock_guard<std::mutex> lock(
        g_mutex);

    result =
        g_virtual_ip;

    return result.c_str();
}
const char *
edgenet_vpn_gateway(void)
{
    static thread_local std::string result;

    std::lock_guard<std::mutex> lock(
        g_mutex);

    result = g_gateway;

    return result.c_str();
}


const char *
edgenet_vpn_protocol(void)
{
    static thread_local std::string result;

    std::lock_guard<std::mutex> lock(
        g_mutex);

    result = g_protocol;

    return result.c_str();
}


const char *
edgenet_vpn_tun_name(void)
{
    static thread_local std::string result;

    std::lock_guard<std::mutex> lock(
        g_mutex);

    result = g_tun_name;

    return result.c_str();
}

} // extern "C"
