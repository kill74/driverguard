use std::env;
use std::io::{self, IsTerminal, Write};
use std::process::Command;
#[cfg(windows)]
use std::process::Output;

fn main() {
    if let Err(error) = run() {
        eprintln!("driverguard: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let command = env::args().nth(1).unwrap_or_else(|| "help".to_owned());
    match command.as_str() {
        "scan" => scan(),
        "plan" => plan(),
        "install" => install(),
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        other => Err(format!("unknown command '{other}'. Run 'driverguard help'.")),
    }
}



fn print_help() {
    println!("DriverGuard: review-first driver inventory and updater\n\nUSAGE:\n  driverguard scan\n  driverguard plan\n  driverguard install\n\nSupported: Windows 10/11, Ubuntu, Debian, Fedora, Arch Linux");
}

#[cfg(windows)]
fn scan() -> Result<(), String> {
    let script = r#"$ErrorActionPreference='Stop'; $board=Get-CimInstance Win32_BaseBoard | Select-Object -First 1 Manufacturer,Product,Version; $devices=Get-CimInstance -ClassName Win32_PnPSignedDriver -Filter "DeviceClass = 'DISPLAY' OR DeviceClass = 'NET' OR DeviceClass = 'BLUETOOTH' OR DeviceClass = 'SYSTEM'" -Property DeviceName,DeviceClass,DriverVersion,Manufacturer,DeviceID | Select-Object DeviceName,DeviceClass,DriverVersion,Manufacturer,DeviceID; [pscustomobject]@{Board=$board;Devices=$devices} | ConvertTo-Json -Depth 4"#;
    print!("{}", run_powershell(script)?);
    Ok(())
}

#[cfg(target_os = "linux")]
fn scan() -> Result<(), String> {
    let distro = linux_distro()?;
    println!("OS: {} ({})", distro.pretty_name, distro.id);
    println!("Motherboard: {} / {}", read_trimmed("/sys/class/dmi/id/board_vendor"), read_trimmed("/sys/class/dmi/id/board_name"));
    let (devices, _) = pci_inventory();
    if devices.is_empty() {
        println!("Driver-related PCI devices: none found (install lspci for readable device names)");
    } else {
        println!("Driver-related PCI devices:");
        for device in devices {
            println!("  {device}");
        }
    }
    println!("Network interfaces:");
    match std::fs::read_dir("/sys/class/net") {
        Ok(entries) => {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name != "lo" {
                    println!("  {name}");
                }
            }
        }
        Err(_) => println!("  unavailable"),
    }
    println!("Bluetooth adapters: {}", if std::path::Path::new("/sys/class/bluetooth").exists() { "detected" } else { "not detected" });
    println!("Note: Linux kernel drivers are managed by the running kernel; device driver versions are not uniformly exposed.");
    Ok(())
}

#[cfg(not(any(windows, target_os = "linux")))]
fn scan() -> Result<(), String> {
    Err("this build supports Windows and Linux only".to_owned())
}

#[cfg(windows)]
fn plan() -> Result<(), String> {
    let script = r#"$ErrorActionPreference='Stop'; $session=New-Object -ComObject Microsoft.Update.Session; $searcher=$session.CreateUpdateSearcher(); $result=$searcher.Search("IsInstalled=0 and Type='Driver'"); if($result.Updates.Count -eq 0){'No applicable Windows Update driver offers found.'} else { for($i=0;$i -lt $result.Updates.Count;$i++){ $update=$result.Updates.Item($i); '{0}. {1}' -f ($i+1),$update.Title } }"#;
    println!("Source: Windows Update Agent (applicable driver updates only)");
    print!("{}", run_powershell(script)?);
    Ok(())
}

#[cfg(target_os = "linux")]
fn plan() -> Result<(), String> {
    let distro = linux_distro()?;
    let (_, gpu_vendor) = pci_inventory();
    let packages = linux_packages(&distro.id, gpu_vendor)?;
    print_linux_plan(&distro, gpu_vendor, &packages);
    Ok(())
}

#[cfg(target_os = "linux")]
fn print_linux_plan(distro: &Distro, gpu_vendor: Option<GpuVendor>, packages: &[String]) {
    println!("OS: {}", distro.pretty_name);
    println!("Source: configured {} repositories only", distro.package_manager);
    if packages.is_empty() {
        println!("No generic package plan is available for this hardware.");
    } else {
        println!("Packages to install/update: {}", packages.join(", "));
        println!("These packages cover distro firmware and supported open GPU userspace drivers, not every board-vendor utility.");
    }
    if gpu_vendor == Some(GpuVendor::Nvidia) {
        println!("NVIDIA GPU detected: no proprietary driver package will be guessed or installed by this version.");
    }
    if distro.id == "arch" {
        println!("Important: installation uses pacman -Syu, a full system upgrade (Arch does not support partial upgrades).");
    }
    println!("No repositories will be added. Run 'driverguard install' to review and confirm changes.");
}

#[cfg(not(any(windows, target_os = "linux")))]
fn plan() -> Result<(), String> {
    Err("this build supports Windows and Linux only".to_owned())
}

#[cfg(windows)]
fn install() -> Result<(), String> {
    if !io::stdin().is_terminal() {
        return Err("interactive confirmation is required; run this command in a terminal".to_owned());
    }
    let script = r#"$ErrorActionPreference='Stop'; $session=New-Object -ComObject Microsoft.Update.Session; $searcher=$session.CreateUpdateSearcher(); $result=$searcher.Search("IsInstalled=0 and Type='Driver'"); if($result.Updates.Count -eq 0){ Write-Output 'No applicable Windows Update driver offers found.'; exit 0 }; Write-Output 'Windows Update driver offers to install:'; for($i=0;$i -lt $result.Updates.Count;$i++){ Write-Output ('  {0}. {1}' -f ($i+1),$result.Updates.Item($i).Title) }; Write-Host 'Install exactly these offers? [y/N] '; $answer=[Console]::ReadLine(); if($answer -notmatch '^(?i:y|yes)$'){ Write-Output 'Cancelled; no changes made.'; exit 0 }; $identity=[Security.Principal.WindowsIdentity]::GetCurrent(); $principal=New-Object Security.Principal.WindowsPrincipal($identity); if(-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){ Write-Error 'Open an elevated terminal to install drivers.'; exit 2 }; for($i=0;$i -lt $result.Updates.Count;$i++){ $update=$result.Updates.Item($i); if(-not $update.EulaAccepted){$update.AcceptEula()} }; $downloader=$session.CreateUpdateDownloader(); $downloader.Updates=$result.Updates; $downloadResult=$downloader.Download(); if($downloadResult.ResultCode -notin 2,3){ throw "Driver download failed (result $($downloadResult.ResultCode))." }; $installer=$session.CreateUpdateInstaller(); $installer.Updates=$result.Updates; $installResult=$installer.Install(); if($installResult.ResultCode -notin 2,3){ throw "Driver installation failed (result $($installResult.ResultCode))." }; if($installResult.ResultCode -eq 3){ Write-Warning 'Windows Update completed with errors; review the update history for per-driver results.' }; Write-Output "Install result: $($installResult.ResultCode)"; Write-Output "Reboot required: $($installResult.RebootRequired)"; if($installResult.RebootRequired){ exit 10 }"#;
    match run_powershell_interactive(script) {
        Ok(status) if status.code() == Some(10) => {
            if confirm("Windows reports a reboot is required. Reboot now?")? {
                let status = Command::new("shutdown").args(["/r", "/t", "0"]).status().map_err(|e| format!("could not start reboot: {e}"))?;
                if !status.success() { return Err("Windows refused the reboot request".to_owned()); }
            }
        }
        Ok(status) if status.success() => {}
        Ok(status) => return Err(format!("Windows driver installation failed (exit code {:?})", status.code())),
        Err(error) => return Err(error),
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn install() -> Result<(), String> {
    let distro = linux_distro()?;
    let (_, gpu_vendor) = pci_inventory();
    let packages = linux_packages(&distro.id, gpu_vendor)?;
    print_linux_plan(&distro, gpu_vendor, &packages);
    if packages.is_empty() {
        return Err("no safe package plan is available; no changes made".to_owned());
    }
    if !confirm("Install/update these packages from the configured distro repositories?")? {
        println!("Cancelled; no changes made.");
        return Ok(());
    }
    let status = match distro.package_manager.as_str() {
        "apt" => {
            let update_status = run_privileged("apt-get", &["update"])?;
            if !update_status.success() {
                return Err(format!("apt-get update failed (exit code {:?}); packages were not installed", update_status.code()));
            }
            let mut args = vec!["install", "-y"];
            args.extend(packages.iter().map(String::as_str));
            run_privileged("apt-get", &args)?
        }
        "dnf" => {
            let mut args = vec!["install", "-y"];
            args.extend(packages.iter().map(String::as_str));
            run_privileged("dnf", &args)?
        }
        "pacman" => {
            let mut args = vec!["-Syu", "--needed"];
            args.extend(packages.iter().map(String::as_str));
            run_privileged("pacman", &args)?
        }
        _ => return Err("unsupported package manager".to_owned()),
    };
    if !status.success() {
        return Err(format!("{} failed (exit code {:?}); review its output for details", distro.package_manager, status.code()));
    }
    if confirm("Installation finished. Reboot now? (recommended after kernel/firmware changes)")? {
        let reboot = run_privileged("systemctl", &["reboot"])?;
        if !reboot.success() {
            return Err("systemctl could not reboot the machine".to_owned());
        }
    }
    Ok(())
}

#[cfg(not(any(windows, target_os = "linux")))]
fn install() -> Result<(), String> {
    Err("this build supports Windows and Linux only".to_owned())
}

fn confirm(question: &str) -> Result<bool, String> {
    if !io::stdin().is_terminal() {
        return Err("interactive confirmation is required; run this command in a terminal".to_owned());
    }
    print!("{question} [y/N] ");
    io::stdout().flush().map_err(|e| e.to_string())?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer).map_err(|e| e.to_string())?;
    Ok(matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes"))
}

#[cfg(windows)]
fn run_powershell(script: &str) -> Result<String, String> {
    let output = run_powershell_status(script)?;
    if !output.status.success() {
        return Err(output_error("PowerShell command failed", &output));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(windows)]
fn run_powershell_status(script: &str) -> Result<Output, String> {
    Command::new("powershell.exe")
        .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", script])
        .output()
        .map_err(|e| format!("could not start Windows PowerShell: {e}"))
}

    #[cfg(windows)]
    fn run_powershell_interactive(script: &str) -> Result<std::process::ExitStatus, String> {
        Command::new("powershell.exe")
        .args(["-NoLogo", "-NoProfile", "-Command", script])
        .status()
        .map_err(|e| format!("could not start Windows PowerShell: {e}"))
    }

#[cfg(windows)]
fn print_output(output: &Output) {
    print!("{}", String::from_utf8_lossy(&output.stdout));
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.trim().is_empty() {
        eprint!("{stderr}");
    }
}

#[cfg(windows)]
fn output_error(context: &str, output: &Output) -> String {
    let detail = String::from_utf8_lossy(&output.stderr);
    format!("{context}: {}", detail.trim())
}

#[cfg(target_os = "linux")]
#[derive(Debug)]
struct Distro {
    id: String,
    pretty_name: String,
    package_manager: String,
}

#[cfg(target_os = "linux")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GpuVendor {
    Amd,
    Intel,
    Nvidia,
    Other,
}

#[cfg(target_os = "linux")]
fn linux_distro() -> Result<Distro, String> {
    let contents = std::fs::read_to_string("/etc/os-release").map_err(|e| format!("cannot read /etc/os-release: {e}"))?;
    let mut id = String::new();
    let mut pretty_name = String::from("Linux");
    for line in contents.lines() {
        if let Some((key, value)) = line.split_once('=') {
            let value = value.trim_matches('"');
            match key {
                "ID" => id = value.to_ascii_lowercase(),
                "PRETTY_NAME" => pretty_name = value.to_owned(),
                _ => {}
            }
        }
    }
    let package_manager = match id.as_str() {
        "ubuntu" | "debian" => "apt",
        "fedora" => "dnf",
        "arch" => "pacman",
        _ => return Err(format!("unsupported Linux distribution '{id}'; supported: Ubuntu, Debian, Fedora, Arch Linux")),
    };
    Ok(Distro { id, pretty_name, package_manager: package_manager.to_owned() })
}

#[cfg(target_os = "linux")]
fn read_trimmed(path: &str) -> String {
    std::fs::read_to_string(path).map(|value| value.trim().to_owned()).unwrap_or_else(|_| "unknown".to_owned())
}

#[cfg(target_os = "linux")]
fn pci_inventory() -> (Vec<String>, Option<GpuVendor>) {
    if let Ok(output) = Command::new("lspci").args(["-nn"]).output() {
        if output.status.success() {
            let all = String::from_utf8_lossy(&output.stdout);
            let mut devices = Vec::new();
            let mut gpu_vendor = None;
            for line in all.lines() {
                let lower = line.to_ascii_lowercase();
                let is_gpu = lower.contains("vga compatible controller") || lower.contains("3d controller") || lower.contains("display controller");
                let relevant = is_gpu || lower.contains("network controller") || lower.contains("ethernet controller") || lower.contains("usb controller") || lower.contains("host bridge");
                if relevant {
                    devices.push(line.trim().to_owned());
                }
                if is_gpu && gpu_vendor.is_none() {
                    gpu_vendor = Some(vendor_from_text(&lower));
                }
            }
            return (devices, gpu_vendor);
        }
    }
    pci_sysfs_inventory()
}

#[cfg(target_os = "linux")]
fn pci_sysfs_inventory() -> (Vec<String>, Option<GpuVendor>) {
    let mut devices = Vec::new();
    let mut gpu_vendor = None;
    let Ok(entries) = std::fs::read_dir("/sys/bus/pci/devices") else { return (devices, gpu_vendor) };
    for entry in entries.flatten() {
        let path = entry.path();
        let class = read_trimmed(&path.join("class").to_string_lossy());
        let relevant = class.starts_with("0x03") || class.starts_with("0x02") || class.starts_with("0x06") || class.starts_with("0x0c03");
        if !relevant { continue; }
        let vendor = read_trimmed(&path.join("vendor").to_string_lossy());
        let device = read_trimmed(&path.join("device").to_string_lossy());
        devices.push(format!("PCI class {class} [{vendor}:{device}]"));
        if class.starts_with("0x03") && gpu_vendor.is_none() {
            gpu_vendor = Some(vendor_from_text(&vendor));
        }
    }
    (devices, gpu_vendor)
}

#[cfg(target_os = "linux")]
fn vendor_from_text(text: &str) -> GpuVendor {
    let has_token = |candidates: &[&str]| {
        text.split(|character: char| !character.is_ascii_alphanumeric())
            .any(|token| candidates.iter().any(|candidate| token.eq_ignore_ascii_case(candidate)))
    };
    if has_token(&["nvidia", "10de"]) {
        GpuVendor::Nvidia
    } else if has_token(&["amd", "ati", "1002"]) {
        GpuVendor::Amd
    } else if has_token(&["intel", "8086"]) {
        GpuVendor::Intel
    } else {
        GpuVendor::Other
    }
}

#[cfg(target_os = "linux")]
fn linux_packages(distro: &str, gpu: Option<GpuVendor>) -> Result<Vec<String>, String> {
    let mut packages = match distro {
        "ubuntu" => vec!["linux-firmware".to_owned()],
        "debian" => vec!["firmware-linux".to_owned()],
        "fedora" => vec!["linux-firmware".to_owned()],
        "arch" => vec!["linux-firmware".to_owned()],
        _ => return Err(format!("no package plan for '{distro}'")),
    };
    match (distro, gpu) {
        ("ubuntu" | "debian" | "fedora", Some(GpuVendor::Amd | GpuVendor::Intel)) => {
            packages.push("mesa-vulkan-drivers".to_owned());
        }
        ("arch", Some(GpuVendor::Amd)) => {
            packages.push("mesa".to_owned());
            packages.push("vulkan-radeon".to_owned());
        }
        ("arch", Some(GpuVendor::Intel)) => {
            packages.push("mesa".to_owned());
            packages.push("vulkan-intel".to_owned());
        }
        _ => {}
    }
    Ok(packages)
}

#[cfg(target_os = "linux")]
fn run_privileged(program: &str, args: &[&str]) -> Result<std::process::ExitStatus, String> {
    let is_root = Command::new("id").arg("-u").output().map(|output| output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "0").unwrap_or(false);
    let status = if is_root {
        Command::new(program).args(args).status()
    } else {
        let mut sudo_args = vec![program];
        sudo_args.extend_from_slice(args);
        Command::new("sudo").args(sudo_args).status()
    };
    status.map_err(|e| format!("could not run {program}: {e}"))
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn nvidia_does_not_select_an_unverified_proprietary_package() {
        assert_eq!(linux_packages("ubuntu", Some(GpuVendor::Nvidia)).unwrap(), vec!["linux-firmware"]);
    }

    #[test]
    fn unknown_gpu_does_not_select_a_gpu_package() {
        assert_eq!(linux_packages("ubuntu", Some(GpuVendor::Other)).unwrap(), vec!["linux-firmware"]);
    }

    #[test]
    fn amd_arch_plan_uses_distro_mesa_packages() {
        assert_eq!(linux_packages("arch", Some(GpuVendor::Amd)).unwrap(), vec!["linux-firmware", "mesa", "vulkan-radeon"]);
    }

    #[test]
    fn vendor_detection_uses_gpu_ids_and_names() {
        assert_eq!(vendor_from_text("NVIDIA Corporation [10de]"), GpuVendor::Nvidia);
        assert_eq!(vendor_from_text("Advanced Micro Devices [1002]"), GpuVendor::Amd);
        assert_eq!(vendor_from_text("Intel Corporation [8086]"), GpuVendor::Intel);
        assert_eq!(vendor_from_text("ATI Technologies"), GpuVendor::Amd);
    }
}
