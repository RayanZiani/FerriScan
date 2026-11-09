# FerriScan — Platform Matrix

Status legend: **W** = working, **P** = partial / documented limitation, **N** = not planned for
v1.0, **T** = to verify during M9.

Fill the "Verified" column with the build SHA during M9 and remove `T` markers.

## Operating systems

| OS | Version | Arch | Status | Verified |
| --- | --- | --- | --- | --- |
| Windows | 10 1809+ | x64 | W (primary) | T |
| Windows | 11 | x64, arm64 | W | T |
| Windows | Server 2019/2022 | x64 | P (no Store, WebView2 bootstrapper) | T |
| Linux | Ubuntu 22.04/24.04 | x64, arm64 | W | T |
| Linux | Fedora 40+ | x64 | W | T |
| Linux | Arch | x64 | W | T |
| Linux | Wayland sessions | x64 | W | T |
| macOS | 11 Big Sur | x64 | W | T |
| macOS | 13/14/15 | arm64, x64 | W | T |

## Capabilities

| Capability | Windows | Linux | macOS | Notes |
| --- | --- | --- | --- | --- |
| Enumerate volumes | `GetLogicalDriveStringsW` | `/proc/self/mountinfo` | `getmntinfo` | |
| Free space | `GetDiskFreeSpaceExW` | `statvfs` | `statfs` | |
| Fast directory read | `GetFileInformationByHandleEx` + `FILE_FULL_DIR_INFO` (64 KiB batches, implemented) | `getdents64` + `statx` | `getattrlistbulk` | |
| Logical size | `WIN32_FIND_DATAW.nFileSize` | `statx.stx_size` | `ATTR_FILE_DATALENGTH` | |
| Allocated size | `FILE_FULL_DIR_INFO.AllocationSize` (exact, implemented) | `statx.stx_blocks * 512` | `ATTR_FILE_ALLOCSIZE` |  |
| Move to trash | `SHFileOperationW` (`FOF_ALLOWUNDO`, implemented) | not implemented | not implemented | Returns Unsupported off Windows |
| Open / show in folder | `ShellExecuteW` (implemented) | not implemented | not implemented | No child process spawned |
| Hidden/system flags | `FILE_ATTRIBUTE_*` | dotfile heuristic | `UF_HIDDEN` / dotfile | Linux has no true hidden bit |
| Symlink target size | `GetFinalPathNameByHandle` optional | `readlink` | `readlink` | Target not followed by default |
| Hardlink detection | file ID via handle (opt-in) | `(dev, ino)` always | `(dev, fileid)` always | |
| Trash | Recycle Bin via `trash` crate | FreeDesktop trash | `~/.Trash` | Network shares may not support trash |
| Reveal in file manager | `explorer /select,` | `xdg-open <parent>` | `open -R` | |
| System theme events | `WM_SETTINGCHANGE` / registry | `gsettings` + DBus (best effort) | KVO on `AppleInterfaceStyle` | Linux fallback: poll every 5 s while focused |
| Accent color | `DwmGetColorizationColor` | none (uses theme token) | none (uses system blue) | |
| High contrast | `SystemParametersInfo(SPI_GETHIGHCONTRAST)` | GTK theme heuristic | `NSWorkspace.accessibilityDisplayShouldIncreaseContrast` | |
| Reduced motion | `SPI_GETCLIENTAREAANIMATION` | GNOME/KDE setting (best effort) | `accessibilityDisplayShouldReduceMotion` | |
| Long paths > 260 | `\\?\` prefix | n/a | n/a | |
| Locked volumes (BitLocker) | shown as locked | n/a | FileVault is transparent | |
| Network volumes | scan read-only, trash disabled | same | same | |
| Removable media hot-plug | `WM_DEVICECHANGE` | `udev` monitor | `NSWorkspace` notifications | v1.0: poll every 5 s is acceptable |

## Known limitations (documented in-app)

1. **Windows allocated size** is computed from logical size and cluster size; it can differ from
   `GetCompressedFileSizeW` on compressed/sparse files by a few clusters. A "report allocated
   size" advanced option uses the exact API (slower).
2. **Linux hidden files** are dotfiles only; the `hidden` attribute concept does not exist.
3. **macOS `/System`** is a read-only signed volume; scans work, actions are blocked.
4. **FAT32/exFAT** have no permissions and no reliable mtime for directories.
5. **NFS/SMB** may report stale or zero timestamps; errors are aggregated in the Issues panel.
6. **WSL** drvfs mounts expose Windows permissions; scans work, trash goes to a Linux `.Trash`.

## CI targets

| Job | Runner |
| --- | --- |
| test-linux | `ubuntu-24.04` |
| test-windows | `windows-2022` |
| test-macos | `macos-14` (arm64) |
| e2e-windows | `windows-2022` + `tauri-driver` + msedgedriver |
| e2e-linux | `ubuntu-22.04` + xvfb |
| bench-nightly | `ubuntu-24.04` and `windows-2022` |
