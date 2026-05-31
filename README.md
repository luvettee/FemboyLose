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
```

Compare the hashes against `SHA256SUMS.txt`.

If the hashes match, the files are identical to the release artifacts produced by GitHub Actions.

## Source commit

Each release lists the exact source commit used for the build.

You can find it in:

- the release notes
- `SHA256SUMS.txt`
- the linked GitHub Actions run

The source commit should exist here:

https://git.femboy.tw/root/FemboyLose

## Reproducing a release

To reproduce a release, clone the source repository and check out the exact commit listed in the release notes:

```powershell
git clone https://git.femboy.tw/root/FemboyLose.git
cd FemboyLose
git checkout <commit-sha>
```

Then follow the reproducible build instructions in the source repository README.

If your local hashes match `SHA256SUMS.txt`, your local build is identical to the published release artifacts.

## Source

All source code lives here:

https://git.femboy.tw/root/FemboyLose
