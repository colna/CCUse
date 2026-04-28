# CCUse FAQ / 常见问题

## Port Issues / 端口问题

### EN: "All ports occupied" error

CCUse tries up to 100 consecutive ports when starting the local proxy. If all are occupied:

1. Check what's using those ports: `lsof -i :8080-8180` (macOS) or `netstat -ano | findstr "8080"` (Windows).
2. Close unnecessary processes or pick a different port range.
3. Restart CCUse.

### ZH: "所有端口被占用" 错误

CCUse 启动代理时会尝试最多 100 个连续端口。如果全部被占用：

1. 检查端口占用：macOS 用 `lsof -i :8080-8180`，Windows 用 `netstat -ano | findstr "8080"`。
2. 关闭不必要的进程或更换端口范围。
3. 重启 CCUse。

---

## WebView2 (Windows) / WebView2 缺失

### EN: "WebView2 Runtime not found" on Windows

CCUse uses the system WebView2 runtime. If it's missing:

1. Download the Evergreen Bootstrapper from [Microsoft](https://developer.microsoft.com/en-us/microsoft-edge/webview2/).
2. Install and restart CCUse.
3. Windows 11 includes WebView2 by default; Windows 10 may need manual installation.

### ZH: Windows 上提示 "WebView2 运行时未找到"

CCUse 使用系统 WebView2 运行时。如果缺失：

1. 从 [Microsoft](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) 下载 Evergreen Bootstrapper。
2. 安装后重启 CCUse。
3. Windows 11 默认包含 WebView2；Windows 10 可能需要手动安装。

---

## macOS Notarization / macOS 公证

### EN: "CCUse cannot be opened because the developer cannot be verified"

If the app is not notarized:

1. Right-click the app icon and select "Open".
2. Click "Open" in the confirmation dialog.
3. Alternatively: System Settings > Privacy & Security > scroll down and click "Open Anyway".

### ZH: "无法打开 CCUse，因为无法验证开发者"

如果应用未公证：

1. 右键点击应用图标，选择"打开"。
2. 在确认对话框中点击"打开"。
3. 或者：系统设置 > 隐私与安全性 > 向下滚动点击"仍然打开"。

---

## Windows SmartScreen / Windows 误报

### EN: "Windows protected your PC"

SmartScreen may flag unsigned apps:

1. Click "More info".
2. Click "Run anyway".
3. This only happens once per version.

### ZH: "Windows 已保护你的电脑"

SmartScreen 可能标记未签名应用：

1. 点击"更多信息"。
2. 点击"仍要运行"。
3. 每个版本只需操作一次。

---

## API Key Security / API Key 安全

### EN: Are my API keys safe?

Yes. CCUse encrypts all API keys with AES-256-GCM using a master key stored in your OS keyring (macOS Keychain / Windows Credential Manager). Keys are never logged or transmitted anywhere except to the original provider's API endpoint.

### ZH: 我的 API Key 安全吗？

安全。CCUse 使用 AES-256-GCM 加密所有 API Key，主密钥存储在操作系统钥匙链中（macOS Keychain / Windows 凭据管理器）。Key 永远不会被记录日志或发送到供应商 API 端点以外的任何地方。

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
