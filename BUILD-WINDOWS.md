# Building Memorata on Windows

This guide explains how to build Memorata on a Windows machine. No administrator privileges are required for compilation.

## Prerequisites

### 1. Install Rust

Download and install Rust using rustup:

```powershell
winget install Rustlang.Rustup
```

Or download directly from: https://rustup.rs/

After installation, restart your terminal and verify:

```powershell
rustc --version
cargo --version
```

### 2. Install Bun

```powershell
powershell -c "irm bun.sh/install.ps1 | iex"
```

Restart your terminal and verify:

```powershell
bun --version
```

### 3. Install Visual Studio Build Tools

This is required for compiling native code (Tauri backend).

1. Download from: https://visualstudio.microsoft.com/visual-cpp-build-tools/
2. Run the installer
3. Select **"Desktop development with C++"** workload
4. Complete installation

> **Note:** You do NOT need the full Visual Studio IDE. The Build Tools are sufficient and free.

### 4. Install WebView2 (usually pre-installed)

Windows 10/11 typically includes WebView2. If not:

- Download from: https://developer.microsoft.com/en-us/microsoft-edge/webview2/

## Build Steps

```powershell
# Clone the repository
git clone https://github.com/cjpais/memorata.git
cd memorata

# Install dependencies
bun install

# Build the application
bun tauri build
```

## Build Output

After a successful build, you'll find:

- **Executable**: `src-tauri\target\release\memorata.exe`
- **MSI installer**: `src-tauri\target\release\bundle\msi\Memorata_0.8.0_x64.msi`

## Running Without Installation

You can run the executable directly without installing:

```powershell
.\src-tauri\target\release\memorata.exe
```

## Troubleshooting

### "linker 'link.exe' not found"

Install Visual Studio Build Tools (see Prerequisites step 3).

### "WebView2 not found"

Install WebView2 runtime from Microsoft.

### Build takes too long

First build compiles all Rust dependencies (10-15 minutes). Subsequent builds are faster.

## Notes

- No administrator privileges needed for compilation
- Only need admin if you want to install the MSI system-wide
- The standalone `.exe` can be copied and run on other Windows machines
