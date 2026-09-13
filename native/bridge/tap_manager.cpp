#include "tap_manager.hpp"

#include <windows.h>
#include <setupapi.h>
#include <devguid.h>

#include <cstdio>
#include <string>
#include <vector>
#include <sstream>

namespace edgenet
{

static constexpr const wchar_t* TAP_COMPONENT_ID = L"tap0901";

static std::string win_error_message(DWORD code)
{
    LPSTR buffer = nullptr;

    DWORD size = FormatMessageA(
        FORMAT_MESSAGE_ALLOCATE_BUFFER |
        FORMAT_MESSAGE_FROM_SYSTEM |
        FORMAT_MESSAGE_IGNORE_INSERTS,
        nullptr,
        code,
        MAKELANGID(LANG_NEUTRAL, SUBLANG_DEFAULT),
        reinterpret_cast<LPSTR>(&buffer),
        0,
        nullptr
    );

    std::string result;

    if (size && buffer)
    {
        result.assign(buffer, size);
        LocalFree(buffer);
    }
    else
    {
        std::ostringstream oss;
        oss << "Windows error " << code;
        result = oss.str();
    }

    return result;
}


static std::wstring exe_directory()
{
    std::vector<wchar_t> buffer(32768);

    DWORD len = GetModuleFileNameW(
        nullptr,
        buffer.data(),
        static_cast<DWORD>(buffer.size())
    );

    if (len == 0 || len >= buffer.size())
        return L".";

    std::wstring path(buffer.data(), len);

    const auto pos = path.find_last_of(L"\\/");

    if (pos == std::wstring::npos)
        return L".";

    return path.substr(0, pos);
}


static std::wstring default_tap_inf()
{
    return exe_directory() +
        L"\\drivers\\tap\\OemVista.inf";
}


TapStatus query_tap_status()
{
    TapStatus result;

    HDEVINFO devs = SetupDiGetClassDevsW(
        &GUID_DEVCLASS_NET,
        nullptr,
        nullptr,
        DIGCF_PRESENT
    );

    if (devs == INVALID_HANDLE_VALUE)
        return result;

    SP_DEVINFO_DATA devinfo{};
    devinfo.cbSize = sizeof(devinfo);

    for (DWORD index = 0;
         SetupDiEnumDeviceInfo(devs, index, &devinfo);
         ++index)
    {
        wchar_t hwids[4096] = {};

        if (!SetupDiGetDeviceRegistryPropertyW(
                devs,
                &devinfo,
                SPDRP_HARDWAREID,
                nullptr,
                reinterpret_cast<PBYTE>(hwids),
                sizeof(hwids),
                nullptr))
        {
            continue;
        }

        const wchar_t* id = hwids;

        while (*id)
        {
            if (_wcsicmp(id, TAP_COMPONENT_ID) == 0)
            {
                ++result.count;
                result.exists = true;

                std::fprintf(
                    stderr,
                    "[EdgeNet TAP] found tap0901 adapter #%u\n",
                    result.count
                );

                break;
            }

            id += wcslen(id) + 1;
        }
    }

    SetupDiDestroyDeviceInfoList(devs);

    return result;
}


static bool remove_registered_device(
    HDEVINFO devs,
    SP_DEVINFO_DATA& devinfo
)
{
    SP_REMOVEDEVICE_PARAMS remove_params{};

    remove_params.ClassInstallHeader.cbSize =
        sizeof(SP_CLASSINSTALL_HEADER);

    remove_params.ClassInstallHeader.InstallFunction =
        DIF_REMOVE;

    remove_params.Scope = DI_REMOVEDEVICE_GLOBAL;
    remove_params.HwProfile = 0;

    if (!SetupDiSetClassInstallParamsW(
            devs,
            &devinfo,
            &remove_params.ClassInstallHeader,
            sizeof(remove_params)))
    {
        return false;
    }

    return SetupDiCallClassInstaller(
        DIF_REMOVE,
        devs,
        &devinfo
    ) != FALSE;
}


bool create_tap_adapter(
    const std::wstring& inf_path,
    std::string& error
)
{
    std::fprintf(
        stderr,
        "[EdgeNet TAP] creating TAP adapter\n"
    );

    DWORD attrs = GetFileAttributesW(inf_path.c_str());

    if (attrs == INVALID_FILE_ATTRIBUTES)
    {
        error =
            "TAP INF not found: drivers/tap/OemVista.inf";

        return false;
    }

    wchar_t full_inf[MAX_PATH * 4] = {};

    DWORD full_len = GetFullPathNameW(
        inf_path.c_str(),
        static_cast<DWORD>(
            sizeof(full_inf) / sizeof(full_inf[0])
        ),
        full_inf,
        nullptr
    );

    if (full_len == 0)
    {
        error =
            "GetFullPathNameW failed: " +
            win_error_message(GetLastError());

        return false;
    }

    std::fwprintf(
        stderr,
        L"[EdgeNet TAP] INF: %ls\n",
        full_inf
    );

    HDEVINFO devs = SetupDiCreateDeviceInfoList(
        &GUID_DEVCLASS_NET,
        nullptr
    );

    if (devs == INVALID_HANDLE_VALUE)
    {
        error =
            "SetupDiCreateDeviceInfoList failed: " +
            win_error_message(GetLastError());

        return false;
    }

    bool registered = false;

    SP_DEVINFO_DATA devinfo{};
    devinfo.cbSize = sizeof(devinfo);

    if (!SetupDiCreateDeviceInfoW(
            devs,
            L"EdgeNet TAP",
            &GUID_DEVCLASS_NET,
            L"EdgeNet TAP Adapter",
            nullptr,
            DICD_GENERATE_ID,
            &devinfo))
    {
        error =
            "SetupDiCreateDeviceInfoW failed: " +
            win_error_message(GetLastError());

        SetupDiDestroyDeviceInfoList(devs);
        return false;
    }

    /*
     * REG_MULTI_SZ:
     *
     * tap0901\0
     * \0
     *
     * 字符串字面量末尾还有隐含 NUL，
     * 所以这里会得到双 NUL。
     */
    const wchar_t hardware_id[] = L"tap0901\0";

    if (!SetupDiSetDeviceRegistryPropertyW(
            devs,
            &devinfo,
            SPDRP_HARDWAREID,
            reinterpret_cast<const BYTE*>(hardware_id),
            sizeof(hardware_id)))
    {
        error =
            "SetupDiSetDeviceRegistryPropertyW(HARDWAREID) failed: " +
            win_error_message(GetLastError());

        SetupDiDestroyDeviceInfoList(devs);
        return false;
    }

    /*
     * 指定只从我们的 OemVista.inf 中寻找兼容驱动。
     */
    SP_DEVINSTALL_PARAMS_W install_params{};
    install_params.cbSize =
        sizeof(install_params);

    if (!SetupDiGetDeviceInstallParamsW(
            devs,
            &devinfo,
            &install_params))
    {
        error =
            "SetupDiGetDeviceInstallParamsW failed: " +
            win_error_message(GetLastError());

        SetupDiDestroyDeviceInfoList(devs);
        return false;
    }

    install_params.Flags |= DI_ENUMSINGLEINF;

    wcsncpy(
        install_params.DriverPath,
        full_inf,
        MAX_PATH - 1
    );

    install_params.DriverPath[MAX_PATH - 1] =
        L'\0';

    if (!SetupDiSetDeviceInstallParamsW(
            devs,
            &devinfo,
            &install_params))
    {
        error =
            "SetupDiSetDeviceInstallParamsW failed: " +
            win_error_message(GetLastError());

        SetupDiDestroyDeviceInfoList(devs);
        return false;
    }

    /*
     * 从 OemVista.inf 构造 Compatible Driver List。
     */
    if (!SetupDiBuildDriverInfoList(
            devs,
            &devinfo,
            SPDIT_COMPATDRIVER))
    {
        error =
            "SetupDiBuildDriverInfoList failed: " +
            win_error_message(GetLastError());

        SetupDiDestroyDeviceInfoList(devs);
        return false;
    }

    SP_DRVINFO_DATA_W driver_info{};
    driver_info.cbSize =
        sizeof(driver_info);

    if (!SetupDiEnumDriverInfoW(
            devs,
            &devinfo,
            SPDIT_COMPATDRIVER,
            0,
            &driver_info))
    {
        error =
            "No compatible tap0901 driver in OemVista.inf: " +
            win_error_message(GetLastError());

        SetupDiDestroyDriverInfoList(
            devs,
            &devinfo,
            SPDIT_COMPATDRIVER
        );

        SetupDiDestroyDeviceInfoList(devs);

        return false;
    }

    std::fwprintf(
        stderr,
        L"[EdgeNet TAP] driver: %ls\n",
        driver_info.Description
    );

    if (!SetupDiSetSelectedDriverW(
            devs,
            &devinfo,
            &driver_info))
    {
        error =
            "SetupDiSetSelectedDriverW failed: " +
            win_error_message(GetLastError());

        SetupDiDestroyDriverInfoList(
            devs,
            &devinfo,
            SPDIT_COMPATDRIVER
        );

        SetupDiDestroyDeviceInfoList(devs);

        return false;
    }

    /*
     * 注册 ROOT 枚举设备节点。
     */
    if (!SetupDiCallClassInstaller(
            DIF_REGISTERDEVICE,
            devs,
            &devinfo))
    {
        error =
            "DIF_REGISTERDEVICE failed: " +
            win_error_message(GetLastError());

        SetupDiDestroyDriverInfoList(
            devs,
            &devinfo,
            SPDIT_COMPATDRIVER
        );

        SetupDiDestroyDeviceInfoList(devs);

        return false;
    }

    registered = true;

    std::fprintf(
        stderr,
        "[EdgeNet TAP] device registered\n"
    );

    /*
     * 安装刚才选择的 TAP 驱动。
     */
    if (!SetupDiCallClassInstaller(
            DIF_INSTALLDEVICE,
            devs,
            &devinfo))
    {
        const DWORD err = GetLastError();

        error =
            "DIF_INSTALLDEVICE failed: " +
            win_error_message(err);

        if (registered)
            remove_registered_device(
                devs,
                devinfo
            );

        SetupDiDestroyDriverInfoList(
            devs,
            &devinfo,
            SPDIT_COMPATDRIVER
        );

        SetupDiDestroyDeviceInfoList(devs);

        return false;
    }

    SetupDiDestroyDriverInfoList(
        devs,
        &devinfo,
        SPDIT_COMPATDRIVER
    );

    SetupDiDestroyDeviceInfoList(devs);

    std::fprintf(
        stderr,
        "[EdgeNet TAP] TAP driver installed\n"
    );

    /*
     * Windows 网络设备注册不是完全同步的。
     * 等待 SetupAPI/PnP/Network Class 注册完成。
     */
    for (int i = 0; i < 40; ++i)
    {
        Sleep(250);

        auto status = query_tap_status();

        if (status.exists)
        {
            std::fprintf(
                stderr,
                "[EdgeNet TAP] TAP ready, count=%u\n",
                status.count
            );

            return true;
        }
    }

    error =
        "TAP installation completed but adapter "
        "did not appear within 10 seconds";

    return false;
}


bool ensure_tap_adapter(
    std::string& error
)
{
    auto status = query_tap_status();

    if (status.exists)
    {
        std::fprintf(
            stderr,
            "[EdgeNet TAP] existing TAP available, count=%u\n",
            status.count
        );

        return true;
    }

    std::fprintf(
        stderr,
        "[EdgeNet TAP] no tap0901 adapter found\n"
    );

    return create_tap_adapter(
        default_tap_inf(),
        error
    );
}

}
