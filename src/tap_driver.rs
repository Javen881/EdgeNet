
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::Duration,
};

include!(concat!(env!("OUT_DIR"), "/embedded_tap.rs"));

const TAP_COMPONENT_ID: &str = "tap0901";

fn tap_present() -> bool {
    let key = r"HKLM\SYSTEM\CurrentControlSet\Control\Class\{4D36E972-E325-11CE-BFC1-08002BE10318}";

    match Command::new("reg.exe")
        .args([
            "query",
            key,
            "/s",
            "/f",
            TAP_COMPONENT_ID,
        ])
        .output()
    {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}

fn temp_driver_dir() -> PathBuf {
    std::env::temp_dir()
        .join("EdgeNet")
        .join("tap")
}

fn extract_driver_files(
    base: &Path,
) -> Result<(), String> {
    if base.exists() {
        let _ = fs::remove_dir_all(base);
    }

    fs::create_dir_all(base)
        .map_err(|e| format!("无法创建 TAP 临时目录: {e}"))?;

    for (relative_name, bytes) in EMBEDDED_TAP_FILES {
        let dest = base.join(relative_name);

        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| {
                    format!(
                        "无法创建驱动目录 {}: {e}",
                        parent.display()
                    )
                })?;
        }

        fs::write(&dest, bytes)
            .map_err(|e| {
                format!(
                    "无法释放 TAP 文件 {}: {e}",
                    dest.display()
                )
            })?;
    }

    Ok(())
}

fn find_file(
    base: &Path,
    filename: &str,
) -> Option<PathBuf> {
    if !base.exists() {
        return None;
    }

    let entries = fs::read_dir(base).ok()?;

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            if let Some(found) =
                find_file(&path, filename)
            {
                return Some(found);
            }
        } else if path
            .file_name()
            .and_then(|x| x.to_str())
            .map(|x| x.eq_ignore_ascii_case(filename))
            .unwrap_or(false)
        {
            return Some(path);
        }
    }

    None
}

fn find_first_inf(base: &Path) -> Option<PathBuf> {
    if !base.exists() {
        return None;
    }

    let entries = fs::read_dir(base).ok()?;

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            if let Some(found) =
                find_first_inf(&path)
            {
                return Some(found);
            }
        } else if path
            .extension()
            .and_then(|x| x.to_str())
            .map(|x| x.eq_ignore_ascii_case("inf"))
            .unwrap_or(false)
        {
            return Some(path);
        }
    }

    None
}

fn wait_for_tap() -> bool {
    for _ in 0..40 {
        if tap_present() {
            return true;
        }

        thread::sleep(
            Duration::from_millis(250)
        );
    }

    false
}

fn install_with_tapinstall(
    tapinstall: &Path,
    inf: &Path,
) -> Result<(), String> {
    let output = Command::new(tapinstall)
        .arg("install")
        .arg(inf)
        .arg(TAP_COMPONENT_ID)
        .output()
        .map_err(|e| {
            format!(
                "无法启动 tapinstall.exe: {e}"
            )
        })?;

    if output.status.success() {
        return Ok(());
    }

    let stdout =
        String::from_utf8_lossy(&output.stdout);

    let stderr =
        String::from_utf8_lossy(&output.stderr);

    Err(format!(
        "TAP 驱动安装失败。\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    ))
}

fn stage_driver_with_pnputil(
    inf: &Path,
) -> Result<(), String> {
    let output = Command::new("pnputil.exe")
        .arg("/add-driver")
        .arg(inf)
        .arg("/install")
        .output()
        .map_err(|e| {
            format!(
                "无法启动 pnputil.exe: {e}"
            )
        })?;

    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "pnputil 安装 TAP 驱动失败:\n{}",
            String::from_utf8_lossy(
                &output.stderr
            )
        ))
    }
}

pub fn ensure_tap_installed()
    -> Result<(), String>
{
    // 已有 TAP，完全不处理。
    if tap_present() {
        return Ok(());
    }

    let temp_dir = temp_driver_dir();

    extract_driver_files(&temp_dir)?;

    let inf =
        find_file(&temp_dir, "OemVista.inf")
            .or_else(|| find_first_inf(&temp_dir))
            .ok_or_else(|| {
                "EdgeNet 内嵌 TAP 驱动中没有找到 INF 文件"
                    .to_string()
            })?;

    /*
     * 优先使用 tapinstall.exe。
     *
     * 原因：
     * pnputil 能把驱动加入 Driver Store，
     * 但对于不存在的 ROOT\\tap0901，
     * 并不保证创建新的 TAP 网络设备实例。
     *
     * tapinstall.exe install xxx.inf tap0901
     * 可以同时创建 root-enumerated TAP adapter。
     */
    if let Some(tapinstall) =
        find_file(&temp_dir, "tapinstall.exe")
    {
        install_with_tapinstall(
            &tapinstall,
            &inf,
        )?;
    } else {
        // 至少先把驱动加入 Driver Store。
        stage_driver_with_pnputil(&inf)?;

        /*
         * 如果你的 C++ tap_manager.cpp 已实现
         * ROOT\tap0901 设备创建，
         * Driver Store 准备好后它可以继续创建。
         *
         * 如果 tap_manager 尚未实现创建，
         * 最终仍建议在 TAP 驱动目录中放入
         * tapinstall.exe。
         */
    }

    if wait_for_tap() {
        let _ = fs::remove_dir_all(&temp_dir);
        return Ok(());
    }

    Err(
        "TAP 驱动文件已释放/安装，但没有发现 tap0901 网络适配器。\
         请确认 drivers/tap 中包含 tapinstall.exe，且 EdgeNet 以管理员身份运行。"
            .to_string()
    )
}


