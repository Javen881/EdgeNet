#include "wintun_manager.hpp"
#include "wintun.h"

#include <windows.h>

#include <cstdio>
#include <string>

namespace edgenet
{

static HMODULE g_wintun_module = nullptr;
static WINTUN_ADAPTER_HANDLE g_adapter = nullptr;

static WINTUN_CREATE_ADAPTER_FUNC* g_WintunCreateAdapter = nullptr;
static WINTUN_OPEN_ADAPTER_FUNC* g_WintunOpenAdapter = nullptr;
static WINTUN_CLOSE_ADAPTER_FUNC* g_WintunCloseAdapter = nullptr;

static bool load_wintun(std::string& error)
{
    if (g_wintun_module != nullptr)
        return true;

    g_wintun_module = LoadLibraryExW(
        L"wintun.dll",
        nullptr,
        LOAD_LIBRARY_SEARCH_APPLICATION_DIR |
        LOAD_LIBRARY_SEARCH_SYSTEM32
    );

    if (g_wintun_module == nullptr)
    {
        error =
            "cannot load wintun.dll, error=" +
            std::to_string(GetLastError());

        return false;
    }

    g_WintunCreateAdapter =
        reinterpret_cast<WINTUN_CREATE_ADAPTER_FUNC*>(
            GetProcAddress(
                g_wintun_module,
                "WintunCreateAdapter"
            )
        );

    g_WintunOpenAdapter =
        reinterpret_cast<WINTUN_OPEN_ADAPTER_FUNC*>(
            GetProcAddress(
                g_wintun_module,
                "WintunOpenAdapter"
            )
        );

    g_WintunCloseAdapter =
        reinterpret_cast<WINTUN_CLOSE_ADAPTER_FUNC*>(
            GetProcAddress(
                g_wintun_module,
                "WintunCloseAdapter"
            )
        );

    if (!g_WintunCreateAdapter)
    {
        error = "missing symbol: WintunCreateAdapter";
        return false;
    }

    if (!g_WintunOpenAdapter)
    {
        error = "missing symbol: WintunOpenAdapter";
        return false;
    }

    if (!g_WintunCloseAdapter)
    {
        error = "missing symbol: WintunCloseAdapter";
        return false;
    }

    return true;
}

bool ensure_wintun_adapter(
    const std::wstring& adapter_name,
    std::string& error)
{
    error.clear();

    if (!load_wintun(error))
        return false;

    // 已经持有 adapter，就不要重复创建。
    if (g_adapter != nullptr)
    {
        std::fprintf(
            stdout,
            "[EdgeNet] Wintun adapter already active\n"
        );

        return true;
    }

    std::fwprintf(
        stdout,
        L"[EdgeNet] Looking for Wintun adapter: %ls\n",
        adapter_name.c_str()
    );

    g_adapter =
        g_WintunOpenAdapter(
            adapter_name.c_str()
        );

    if (g_adapter != nullptr)
    {
        std::fwprintf(
            stdout,
            L"[EdgeNet] Existing Wintun adapter found: %ls\n",
            adapter_name.c_str()
        );

        return true;
    }

    std::fwprintf(
        stdout,
        L"[EdgeNet] Creating Wintun adapter: %ls\n",
        adapter_name.c_str()
    );

    SetLastError(ERROR_SUCCESS);

    g_adapter =
        g_WintunCreateAdapter(
            adapter_name.c_str(),
            L"EdgeNet",
            nullptr
        );

    if (g_adapter == nullptr)
    {
        DWORD err = GetLastError();

        error =
            "WintunCreateAdapter failed, error=" +
            std::to_string(err);

        return false;
    }

    std::fwprintf(
        stdout,
        L"[EdgeNet] Wintun adapter created successfully: %ls\n",
        adapter_name.c_str()
    );

    // 关键：
    // 这里绝对不要调用 WintunCloseAdapter。
    // 这个 handle 必须在 VPN 整个生命周期内保持。

    return true;
}

void release_wintun_adapter()
{
    if (g_adapter != nullptr)
    {
        std::fprintf(
            stdout,
            "[EdgeNet] Releasing Wintun adapter\n"
        );

        g_WintunCloseAdapter(g_adapter);
        g_adapter = nullptr;
    }

    if (g_wintun_module != nullptr)
    {
        FreeLibrary(g_wintun_module);
        g_wintun_module = nullptr;
    }

    g_WintunCreateAdapter = nullptr;
    g_WintunOpenAdapter = nullptr;
    g_WintunCloseAdapter = nullptr;
}

} // namespace edgenet
