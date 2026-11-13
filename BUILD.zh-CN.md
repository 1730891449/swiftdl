# 构建 SwiftDL 中文版

## GitHub Actions 构建（无需在本机安装 Rust）

将汉化代码及 `.github/workflows/build-windows.yml` 推送到自己的仓库的
`zh-CN` 分支后，`Build Windows Chinese Installer` 会自动运行。
也可以在工作流出现在默认分支后，从 Actions 页面点击 Run workflow 手动运行。

打开 Actions → Build Windows Chinese Installer → 本次运行，构建成功后在
Artifacts 下载 `SwiftDL-zh-CN-Windows-x64`，解压获得安装程序。
工作流会先构建前端，再运行 Rust 测试，最后生成 NSIS 安装包。
此工作流只上传构建附件，不创建或发布 Release，无需额外密钥或写入权限。

## Windows 本地构建

需要 Rust stable MSVC、Visual Studio C++ Build Tools、Windows SDK、
Node.js 22 和 WebView2。安装后重新打开 PowerShell：

```powershell
cd D:\codex\1\swiftdl\app
npm ci
npm run build
cd ..
cargo test --workspace --locked
cd app\src-tauri
npx --yes @tauri-apps/cli@2 build --bundles nsis
```

当前仓库使用 Cargo workspace，因此默认输出在仓库根目录的
`target/release/bundle/nsis/`，不是 `app/src-tauri/target/`。

## 汉化范围与验证

已翻译桌面主界面、下载窗口、设置、浏览器扩展、远程控制页面、CLI 文案、
常用后端提示和系统通知；NSIS 安装器使用简体中文。
内部状态值、分类标识、命令参数、接口和目录名保持原有值。
第三方库、操作系统及远端网站返回的原始错误可能仍为英文。

本地已通过 TypeScript 检查、Vite 生产构建及扩展脚本语法检查。
本地没有 Rust 工具链，Rust 测试及安装包构建须以 Actions 结果为准。
