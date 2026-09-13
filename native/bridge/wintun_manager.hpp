#pragma once

#include <string>

namespace edgenet
{

bool ensure_wintun_adapter(
    const std::wstring& adapter_name,
    std::string& error
);

void release_wintun_adapter();

}
