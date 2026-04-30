# CCUse FAQ / 常见问题

## Port Issues / 端口问题

### EN: "All ports occupied" error

CCUse listens only on `127.0.0.1`. It tries `8787` first, then probes 100 consecutive loopback ports through `8886`. If all are occupied:

1. Check what's using the preferred port: `lsof -nP -iTCP:8787 -sTCP:LISTEN` (macOS) or `netstat -ano | findstr :8787` (Windows PowerShell).
2. Close unnecessary processes, or free one of the ports from `8787` through `8886`.
3. Restart CCUse or use Restart Proxy, then copy the exact Base URL shown in the Local API card.

### ZH: "所有端口被占用" 错误

CCUse 只监听 `127.0.0.1`。它会优先尝试 `8787`，然后继续探测 100 个 loopback 端口，直到 `8886`。如果全部被占用：

1. 检查首选端口占用：macOS 用 `lsof -nP -iTCP:8787 -sTCP:LISTEN`，Windows PowerShell 用 `netstat -ano | findstr :8787`。
2. 关闭不必要的进程，或释放 `8787` 到 `8886` 中的一个端口。
3. 重启 CCUse 或使用 Restart Proxy，然后复制 Local API 卡片显示的准确 Base URL。

---

## WebView2 (Windows) / WebView2 缺失

### EN: "WebView2 Runtime not found" on Windows

CCUse uses the Microsoft Edge WebView2 Runtime. The Windows installer is configured with `webviewInstallMode: downloadBootstrapper`, so it can fetch WebView2 during install when network policy allows it.

If it is still missing:

1. Download the Evergreen Runtime from [Microsoft](https://developer.microsoft.com/en-us/microsoft-edge/webview2/).
2. Re-run `CCUse_<version>_x64-setup.exe`.
3. Windows 11 usually includes WebView2; Windows 10 or locked-down corporate images may need manual installation or IT allow-listing.

### ZH: Windows 上提示 "WebView2 运行时未找到"

CCUse 使用 Microsoft Edge WebView2 Runtime。Windows 安装器已配置 `webviewInstallMode: downloadBootstrapper`，在网络策略允许时会在安装阶段下载 WebView2。

如果仍然缺失：

1. 从 [Microsoft](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) 下载 Evergreen Runtime。
2. 重新运行 `CCUse_<version>_x64-setup.exe`。
3. Windows 11 通常自带 WebView2；Windows 10 或受管控的企业镜像可能需要手动安装或 IT 放行。

---

## macOS Notarization / macOS 公证

### EN: "CCUse cannot be opened because the developer cannot be verified"

Official release builds are intended to be signed and notarized when Apple signing secrets are enabled in CI. If macOS still blocks the app:

1. Confirm the file came from the CCUse GitHub Release page.
2. Use `CCUse_<version>_aarch64.dmg` for Apple Silicon or `CCUse_<version>_x64.dmg` for Intel.
3. Right-click the app icon and select "Open", or use System Settings > Privacy & Security > Open Anyway.

### ZH: "无法打开 CCUse，因为无法验证开发者"

正式 Release 构建在 CI 启用 Apple signing secrets 时会进行 Developer ID 签名和公证。如果 macOS 仍然阻止打开：

1. 确认文件来自 CCUse GitHub Release 页面。
2. Apple Silicon 使用 `CCUse_<version>_aarch64.dmg`，Intel 使用 `CCUse_<version>_x64.dmg`。
3. 右键点击应用图标选择"打开"，或进入系统设置 > 隐私与安全性 > 仍然打开。

---

## Windows SmartScreen / Windows 误报

### EN: "Windows protected your PC"

SmartScreen or Defender may warn for a new or unsigned build before it has enough reputation.

1. Confirm the installer name is `CCUse_<version>_x64-setup.exe`.
2. Download it from the CCUse GitHub Release page, not a mirror.
3. If you trust the release, click "More info" and then "Run anyway". In managed environments, ask IT to allow-list the signed installer or release hash instead of disabling Defender globally.

### ZH: "Windows 已保护你的电脑"

SmartScreen 或 Defender 可能会在新版本或尚未建立声誉的构建上弹出警告。

1. 确认安装器名称是 `CCUse_<version>_x64-setup.exe`。
2. 从 CCUse GitHub Release 页面下载，不要使用镜像站。
3. 如果你确认信任该 Release，可以点击"更多信息"和"仍要运行"。企业环境中，建议让 IT allow-list 已签名安装器或 release hash，而不是全局关闭 Defender。

---

## API Key Security / API Key 安全

### EN: Are my API keys safe?

Yes. CCUse encrypts all API keys with AES-256-GCM using a master key stored in your OS keyring (macOS Keychain / Windows Credential Manager). Keys are never logged or transmitted anywhere except to the original provider's API endpoint.

### ZH: 我的 API Key 安全吗？

安全。CCUse 使用 AES-256-GCM 加密所有 API Key，主密钥存储在操作系统钥匙链中（macOS Keychain / Windows 凭据管理器）。Key 永远不会被记录日志或发送到供应商 API 端点以外的任何地方。

---

## Local API Auth / 本地 API 鉴权

### EN: Client gets 401 Unauthorized

The local proxy accepts either `Authorization: Bearer sk-local-...` or `x-api-key: sk-local-...`. If you regenerate the local key, the old key stops working. Copy the new key from the Local API card and update Cursor, Claude Desktop, or scripts that call CCUse.

### ZH: 客户端返回 401 Unauthorized

本地代理接受 `Authorization: Bearer sk-local-...` 或 `x-api-key: sk-local-...`。重新生成本地 key 后，旧 key 会失效。请从 Local API 卡片复制新 key，并更新 Cursor、Claude Desktop 或调用 CCUse 的脚本。

---

## No Providers Configured / 无可用供应商

### EN: Client gets providers_not_configured

`providers_not_configured` means the proxy is running but no enabled provider can serve the request. Enable at least one provider, verify the upstream API key and Base URL, and confirm the requested model maps to a provider model when model mapping is required.

### ZH: 客户端返回 providers_not_configured

`providers_not_configured` 表示代理已经启动，但当前没有可处理请求的已启用供应商。请启用至少一个供应商，检查上游 API key 和 Base URL，并在需要时确认请求模型已映射到供应商模型。

---

## Proxy Not Starting / 代理未启动

### EN: Dashboard shows "Not Running"

1. Check if another instance of CCUse is already running (check system tray).
2. Try "Restart Proxy" from the tray menu.
3. Check the application logs in your app data directory.

### ZH: 仪表盘显示"未运行"

1. 检查是否已有 CCUse 实例在运行（查看系统托盘）。
2. 尝试从托盘菜单"重启代理"。
3. 检查应用数据目录中的日志。

---

## Config Import Failed / 配置导入失败

### EN: "Decryption failed" when importing

This means the password is incorrect or the file is corrupted. Double-check the password you used when exporting. Config files use strong encryption (scrypt + AES-256-GCM) and cannot be recovered without the correct password.

### ZH: 导入时提示"解密失败"

这意味着口令不正确或文件已损坏。请确认导出时使用的口令。配置文件使用强加密（scrypt + AES-256-GCM），无法在没有正确口令的情况下恢复。
