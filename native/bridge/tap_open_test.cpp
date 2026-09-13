#include <windows.h>
#include <cstdio>

int main()
{
    const wchar_t* path =
        L"\\\\.\\Global\\{31C061F0-9A86-4AF5-9E91-28D03EE82C7A}.tap";

    wprintf(L"Opening TAP device:\n%ls\n\n", path);

    HANDLE h = CreateFileW(
        path,
        GENERIC_READ | GENERIC_WRITE,
        0,
        nullptr,
        OPEN_EXISTING,
        FILE_ATTRIBUTE_SYSTEM | FILE_FLAG_OVERLAPPED,
        nullptr
    );

    if (h == INVALID_HANDLE_VALUE)
    {
        DWORD err = GetLastError();

        std::printf(
            "CreateFileW FAILED, error=%lu\n",
            static_cast<unsigned long>(err)
        );

        return 1;
    }

    std::printf("CreateFileW SUCCESS\n");
    std::printf("TAP handle = %p\n", h);

    CloseHandle(h);

    return 0;
}
