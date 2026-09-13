#pragma once

#include <string>

namespace edgenet
{

struct TapStatus
{
    bool exists = false;
    unsigned int count = 0;
};

TapStatus query_tap_status();

bool create_tap_adapter(
    const std::wstring& inf_path,
    std::string& error
);

bool ensure_tap_adapter(
    std::string& error
);

}
