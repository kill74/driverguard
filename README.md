# DriverGuard

DriverGuard is a small, review-first command-line tool for inspecting driver-related hardware and applying a limited set of operating-system-provided driver and firmware updates.

Its priority is predictable behavior over broad hardware coverage. It does not download drivers from arbitrary websites, add software repositories, or guess proprietary packages. **DriverGuard is not a replacement for your computer manufacturer's support tools, a complete driver inventory, or a backup and recovery plan.**

## What it supports

| Platform | Inventory (`scan`) | Review (`plan`) | Apply (`install`) |
| --- | --- | --- | --- |
| Windows 10 and 11 | Motherboard and selected signed Plug and Play drivers | Applicable driver offers exposed by Windows Update | Downloads and installs those Windows Update offers |
| Ubuntu | Board details, selected PCI devices, interfaces, Bluetooth presence | Firmware and, for detected AMD/Intel graphics, a Mesa Vulkan package | Installs the listed packages through APT |
| Debian | Same Linux inventory | Firmware and, for detected AMD/Intel graphics, a Mesa Vulkan package | Installs the listed packages through APT |
| Fedora | Same Linux inventory | Firmware and, for detected AMD/Intel graphics, a Mesa Vulkan package | Installs the listed packages through DNF |
| Arch Linux | Same Linux inventory | Firmware and, for detected AMD/Intel graphics, Mesa/Vulkan packages | Installs through `pacman -Syu` (a full system upgrade) |

Linux distribution detection is based on the exact `ID` in `/etc/os-release`. Other distributions and derivatives are not currently supported, even if they use APT, DNF, or pacman.

## What it does not do

- It does not promise to find every device or every available driver.
- On Linux, device kernel drivers are generally supplied and managed by the running kernel. DriverGuard does not independently install or switch kernel modules.
- The Linux package plan covers selected distribution firmware and open graphics userspace packages; it does not cover every device, motherboard utility, Wi-Fi firmware edge case, or vendor-specific driver.
- It does not choose or install a proprietary NVIDIA driver. NVIDIA is detected, but the proprietary package is deliberately not guessed.
- It does not add repositories, install third-party downloads, or change firmware/BIOS settings.
- It does not automatically reboot the machine.
- It does not guarantee that an update will be compatible with every system configuration. Always review the plan and maintain a way to recover your system.

## Commands

Run commands from a terminal:

```text
driverguard help
driverguard scan
driverguard plan
driverguard install
```

If running from the repository before installing the executable, use Cargo:

```text
cargo run -- scan
cargo run -- plan
cargo run -- install
```

### `scan`

Performs a read-only hardware inventory and does not request administrator/root privileges.

- **Windows:** outputs JSON containing the first motherboard record and signed driver records for display, network, Bluetooth, and system device classes. It uses Windows Management Instrumentation through PowerShell and filters the selected device classes in the CIM query, requesting only the fields included in the output. The performance benefit depends on the system and WMI provider; scan times are not guaranteed.
- **Linux:** prints distribution and motherboard information, selected PCI devices, non-loopback network interface names, and whether a Bluetooth sysfs directory is present. If `lspci` is installed, PCI device names are more readable. Without it, DriverGuard falls back to PCI IDs from sysfs.

The inventory is intentionally selective and is not a complete list of all hardware or installed driver versions.

### `plan`

Shows the update source and proposed updates/packages without installing them.

- **Windows:** queries the Windows Update Agent for currently applicable, uninstalled driver updates. This is a live query and requires Windows Update components to be available.
- **Linux:** reports a fixed package plan based on the detected distribution and, when possible, the detected graphics vendor. It uses only repositories already configured on the system.

A plan is informational. Package availability and update results can change before a later install.

### `install`

Shows the applicable update/package plan and requires an explicit interactive `y` or `yes` confirmation before making changes. Any other response cancels. It refuses to run without an interactive terminal.

- **Windows:** requires an elevated terminal to install. DriverGuard downloads and installs the currently offered Windows Update driver updates after confirmation. It may accept update license terms as part of the Windows Update Agent installation process. If Windows reports a reboot is required, DriverGuard asks separately before rebooting.
- **Linux:** runs the distribution's package manager, using `sudo` when not already root. On Ubuntu/Debian it refreshes APT metadata before installing packages. On Arch it runs `pacman -Syu --needed`, which upgrades the full system as required by Arch's supported update model. After package installation, it asks separately whether to reboot.

The confirmation is an important safety boundary, not a guarantee that a package manager or driver update cannot cause problems. Read the displayed plan and the package manager's output before proceeding.

## Installation

### Download a prebuilt release

Open this repository's GitHub **Releases** page and download the archive for your system:

- Windows x64: `driverguard-windows-x86_64.zip`
- Linux x64: `driverguard-linux-x86_64.tar.gz`

Each release also includes `SHA256SUMS`. Download that file alongside the archive and verify the checksum before extracting. The checksum detects an incomplete or changed download; it is not a digital signature. Download the archive and checksum from the same release page.

On Windows, verify the archive against its entry in `SHA256SUMS` before extracting:

```powershell
$line = Get-Content .\SHA256SUMS | Where-Object { $_ -match '^[0-9a-fA-F]{64}\s+\*?driverguard-windows-x86_64\.zip$' } | Select-Object -First 1
if (-not $line) { throw "Windows archive checksum is missing from SHA256SUMS." }
$expected = [regex]::Match($line, '^[0-9a-fA-F]{64}').Value
$actual = (Get-FileHash .\driverguard-windows-x86_64.zip -Algorithm SHA256).Hash
if ($actual -ne $expected) { throw "Checksum verification failed." }
Write-Output "Checksum verified."
Expand-Archive .\driverguard-windows-x86_64.zip -DestinationPath .\driverguard
.\driverguard\driverguard.exe scan
```

On Linux, put both the archive and `SHA256SUMS` in the same directory, then run:

```sh
sha256sum --check --ignore-missing SHA256SUMS
tar -xzf driverguard-linux-x86_64.tar.gz
chmod +x ./driverguard
./driverguard scan
```

The published Linux executable targets x86_64 GNU/Linux and is built on a GitHub-hosted Ubuntu runner. Compatibility depends on the system's C library and runtime environment; it is not guaranteed for every Linux distribution. If it does not run on your system, build from source on that system using the instructions below.

### Build from source

Install the stable Rust toolchain using the official Rust installation instructions. From the project directory, build and test:

```text
cargo build --release
cargo test
```

The release executable is:

- Windows: `target\release\driverguard.exe`
- Linux: `target\release\driverguard`

You can run it directly from the project directory:

```text
target\release\driverguard.exe scan
```

On Linux:

```text
./target/release/driverguard scan
```

To run `driverguard` from any directory, put the executable in a directory on your `PATH`. Keep the executable in a trusted, user-controlled location.

### Requirements

- Rust stable and Cargo are needed only to build from source.
- Windows use requires Windows PowerShell and the Windows Update Agent. Installing updates requires an elevated terminal.
- Linux use requires access to `/etc/os-release` and the relevant `/sys` entries. `lspci` is optional and improves device names.
- Installing on Linux requires the applicable package manager (`apt-get`, `dnf`, or `pacman`) and either root access or working `sudo` access.
- Network access may be required by Windows Update and the Linux package manager.

### Maintainer release process

The CI workflow runs tests and an optimized build on Windows and Linux for pushes and pull requests. The release workflow also runs tests on both platforms. To publish downloadable binaries, update the package version in `Cargo.toml` and push a matching version tag such as `vX.Y.Z` to GitHub. The release workflow checks that the tag matches the package version, builds Windows x64 and Linux x64 archives, generates SHA-256 checksums, and creates a GitHub release. A mismatched tag fails before packaging.

## Recommended workflow

1. Run `driverguard scan` and check that the detected platform and hardware look reasonable.
2. Run `driverguard plan` and read every proposed update or package.
3. Before installing, save important work, ensure you have a recent backup, and make sure you can recover if a driver update causes a boot, display, or network problem.
4. Run `driverguard install` in a terminal and confirm only if you intend to apply the displayed plan.
5. Review the operating system/package manager output. Reboot only when you are ready and accept the separate reboot prompt.

For production, shared, or business-critical machines, follow your organization's update and recovery procedures instead of applying changes without review.

## Reliability and safety notes

- **No unattended changes:** installation requires an interactive confirmation. Piped or scripted installation is rejected.
- **No arbitrary source selection:** Windows uses Windows Update; Linux uses the configured distribution repositories.
- **Conservative Linux graphics handling:** only the package mappings in the table above are selected. Unknown graphics vendors receive no graphics-specific package, and NVIDIA proprietary packages are never inferred.
- **System-level changes:** driver, firmware, and operating-system package updates can require a reboot or affect hardware behavior. Keep recovery media or another recovery path available.
- **Arch Linux updates:** the `install` command performs a full system upgrade, not a driver-only upgrade. Review the entire package-manager transaction before accepting it.
- **Platform coverage:** Windows 10/11 and the four named Linux distributions are the documented targets. Behavior on other systems, Linux derivatives, unusual hardware, or customized package sources is not guaranteed.
- **No warranty:** this project is provided without a guarantee that a proposed update is appropriate for a particular device or configuration.

## Troubleshooting

### `unknown command`

Run `driverguard help`. The supported commands are `scan`, `plan`, and `install`.

### `this build supports Windows and Linux only`

DriverGuard has no implementation for the current operating system. Build and run it on a supported Windows or Linux platform.

### Linux says the distribution is unsupported

DriverGuard currently recognizes exact IDs for Ubuntu, Debian, Fedora, and Arch Linux. A derivative distribution may report its own ID and therefore be rejected; do not edit its package sources merely to make DriverGuard accept it.

### Linux reports no readable PCI device names

Install the distribution's `pciutils` package to provide `lspci`, then run `scan` again. DriverGuard attempts a sysfs fallback, but the fallback displays numeric PCI IDs rather than descriptive names.

### Linux package installation fails

Read the package manager's error output. Check network access, configured repositories, package availability for your release, and whether `sudo`/root privileges are available. For Debian in particular, firmware package availability depends on the system's configured repository components. DriverGuard does not modify repository configuration.

### Windows cannot query Windows Update

Check that Windows Update is available and that the machine can reach its configured update service. Organizational policy, network restrictions, or Windows Update service problems may prevent results. DriverGuard does not bypass those policies.

### Installation is cancelled or says a terminal is required

Run `install` directly in an interactive terminal and answer `y` or `yes` at the confirmation prompt. Windows also requires an elevated terminal to install. Linux may prompt for `sudo` credentials.

### Installation completed but hardware still does not work

DriverGuard's package plan is limited and cannot diagnose every device. Check the device manufacturer's official support information and your operating system's device or system logs. Use the recovery procedure appropriate for your system if the update caused a regression.

## Development

The project is a single Rust binary with no third-party Cargo dependencies declared in [`Cargo.toml`](./Cargo.toml). Platform-specific behavior is conditionally compiled in [`src/main.rs`](./src/main.rs).

Run the available unit tests with:

```text
cargo test
```

The current unit tests cover Linux package selection and graphics-vendor detection and are compiled only for Linux. Passing them does not validate live hardware detection, external package repositories, Windows Update behavior, or a real system installation. Test on a disposable or recoverable system before relying on changes in an important environment.
