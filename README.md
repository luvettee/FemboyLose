# FemboyLose release repo

This repository contains no source code.

Releases are built automatically by GitHub Actions from the public source repository:

https://git.femboy.tw/root/FemboyLose

This repository only exists to provide GitHub-hosted release artifacts, release hashes, and build logs.

## Download

Download the latest release from the [Releases](../../releases/latest) page.

Download all three binaries and place them in the same folder:

| File | Purpose |
|------|---------|
| `neverlose.dll` | Main module, x86 |
| `injector.exe` | DLL injector, x86, requires administrator permissions |
| `neverlose-server.exe` | Local server, x64 |

## Verifying a release

Every release includes `SHA256SUMS.txt`.

To verify the downloaded files, place all three binaries in the same folder and run:

```powershell
Get-FileHash neverlose.dll, injector.exe, neverlose-server.exe -Algorithm SHA256
