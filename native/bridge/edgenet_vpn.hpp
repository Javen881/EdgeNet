#pragma once

#ifdef __cplusplus
extern "C" {
#endif

enum EdgeNetVpnState
{
    EDGENET_VPN_DISCONNECTED  = 0,
    EDGENET_VPN_CONNECTING    = 1,
    EDGENET_VPN_CONNECTED     = 2,
    EDGENET_VPN_DISCONNECTING = 3,
    EDGENET_VPN_ERROR         = -1
};

int edgenet_vpn_connect(const char *config);
int edgenet_vpn_disconnect(void);
int edgenet_vpn_status(void);

const char *edgenet_vpn_last_error(void);
const char *edgenet_vpn_virtual_ip(void);
const char *edgenet_vpn_gateway(void);
const char *edgenet_vpn_protocol(void);
const char *edgenet_vpn_tun_name(void);

#ifdef __cplusplus
}
#endif
