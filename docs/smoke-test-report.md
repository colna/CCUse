# CCUse v1.0.1 Smoke Test Report

- **Date**: 2026-04-30 12:13 Asia/Shanghai
- **Release**: `v1.0.1`
- **Release URL**: https://github.com/colna/CCUse/releases/tag/v1.0.1
- **Overall automated gate**: PASS
- **Manual clean VM installer smoke**: Not executed in this environment

## Release Assets

| Platform            | Required asset              | State    |            Size |
| ------------------- | --------------------------- | -------- | --------------: |
| macOS Apple Silicon | `CCUse_1.0.1_aarch64.dmg`   | uploaded | 5,300,702 bytes |
| macOS Intel         | `CCUse_1.0.1_x64.dmg`       | uploaded | 5,435,013 bytes |
| Windows x64         | `CCUse_1.0.1_x64-setup.exe` | uploaded | 2,759,509 bytes |

The GitHub Release is not draft and not prerelease.

## Automated Verification

| Check                                    | Result | Evidence                                                                                                                                  |
| ---------------------------------------- | ------ | ----------------------------------------------------------------------------------------------------------------------------------------- |
| Release workflow                         | PASS   | Run `25146771380`: https://github.com/colna/CCUse/actions/runs/25146771380                                                                |
| macOS Apple Silicon package build/upload | PASS   | Job `Build macOS (Apple Silicon)` completed successfully and uploaded `CCUse_1.0.1_aarch64.dmg`.                                          |
| macOS Intel package build/upload         | PASS   | Job `Build macOS (Intel)` completed successfully and uploaded `CCUse_1.0.1_x64.dmg`.                                                      |
| Windows x64 asset presence               | PASS   | Existing `CCUse_1.0.1_x64-setup.exe` was detected and retained.                                                                           |
| Desktop Tauri E2E                        | PASS   | Run `25146771404`: https://github.com/colna/CCUse/actions/runs/25146771404                                                                |
| Linux real Tauri shell E2E               | PASS   | Ubuntu job started the real app with `tauri-driver`, added a mock provider, sent `/v1/chat/completions`, and verified dashboard metrics.  |
| Windows real Tauri shell E2E             | PASS   | Windows job started the real app with `tauri-driver`, added a mock provider, sent `/v1/chat/completions`, and verified dashboard metrics. |

## Clean Install Matrix

| Target              | Automated coverage                                                                                                             | Manual clean install status                                                                                                                                   |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| macOS Apple Silicon | Release `.dmg` built and uploaded on a clean GitHub-hosted macOS runner.                                                       | Not executed. This session does not have an independent clean macOS Apple Silicon VM, and desktop `tauri-driver` does not support macOS WebDriver automation. |
| macOS Intel         | Release `.dmg` built and uploaded on a clean GitHub-hosted macOS runner.                                                       | Not executed. This session does not have an independent clean macOS Intel VM, and desktop `tauri-driver` does not support macOS WebDriver automation.         |
| Windows x64         | Release `.exe` is present; real Tauri shell E2E passed on a clean GitHub-hosted Windows runner using the built desktop binary. | Not executed against the NSIS installer. This session does not have an independent clean Windows VM with interactive installer access.                        |

## Scope Notes

- The automated E2E path covers first app launch, adding a provider, local proxy request forwarding, and dashboard metric update in real Tauri shells where supported by `tauri-driver`.
- Tray visibility, close-to-tray behavior, Gatekeeper, SmartScreen, and manual installer UX were not manually validated in this environment.
- The `v1.0.1` macOS packages were produced as unsigned recovery builds because project-specific signing variables were not enabled.
