# yml-diff Windows 安装脚本
# 使用方法: iex ((New-Object System.Net.WebClient).DownloadString('https://raw.githubusercontent.com/yourusername/yml-diff/main/install.ps1'))
param(
    [string]$Version = "latest"
)

$ErrorActionPreference = "Stop"

$REPO = "JimDa/yml-diff"  # 替换为你的实际 GitHub 用户名/仓库名

function Get-LatestVersion {
    try {
        $response = Invoke-RestMethod -Uri "https://api.github.com/repos/$REPO/releases/latest"
        return $response.tag_name
    }
    catch {
        Write-Error "无法获取最新版本: $_"
    }
}

function Install-YamlDiff {
    Write-Host "🔍 检测平台..." -ForegroundColor Blue
    
    if ($Version -eq "latest") {
        Write-Host "🔍 获取最新版本..." -ForegroundColor Blue
        $Version = Get-LatestVersion
    }
    
    Write-Host "版本: $Version" -ForegroundColor Green
    
    $BINARY_NAME = "yml-diff-windows-x86_64.exe"
    $DOWNLOAD_URL = "https://github.com/$REPO/releases/download/$Version/$BINARY_NAME"
    
    Write-Host "⬇️  下载 $DOWNLOAD_URL" -ForegroundColor Blue
    
    # 创建临时目录
    $TEMP_DIR = New-TemporaryFile | ForEach-Object { Remove-Item $_; New-Item -ItemType Directory -Path $_ }
    
    try {
        # 下载二进制文件
        $TEMP_FILE = Join-Path $TEMP_DIR "yml-diff.exe"
        Invoke-WebRequest -Uri $DOWNLOAD_URL -OutFile $TEMP_FILE
        
        # 确定安装目录
        $INSTALL_DIR = "$env:USERPROFILE\.local\bin"
        if (!(Test-Path $INSTALL_DIR)) {
            New-Item -ItemType Directory -Path $INSTALL_DIR -Force | Out-Null
        }
        
        # 安装
        Write-Host "📦 安装到 $INSTALL_DIR" -ForegroundColor Blue
        Copy-Item $TEMP_FILE -Destination "$INSTALL_DIR\yml-diff.exe"
        
        # 添加到 PATH
        $USER_PATH = [Environment]::GetEnvironmentVariable("PATH", "User")
        if ($USER_PATH -notlike "*$INSTALL_DIR*") {
            $NEW_PATH = "$USER_PATH;$INSTALL_DIR"
            [Environment]::SetEnvironmentVariable("PATH", $NEW_PATH, "User")
            Write-Host "✅ 已添加到 PATH: $INSTALL_DIR" -ForegroundColor Green
        }
        
        Write-Host "✅ yml-diff 安装成功!" -ForegroundColor Green
        Write-Host "💡 使用方法: yml-diff --old config_v1.yml --new config_v2.yml" -ForegroundColor Yellow
        Write-Host "⚠️  请重启终端或运行 refreshenv 来更新 PATH" -ForegroundColor Yellow
        
    }
    finally {
        # 清理临时文件
        Remove-Item $TEMP_DIR -Recurse -Force -ErrorAction SilentlyContinue
    }
}

Install-YamlDiff