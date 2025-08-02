#!/bin/bash

# yml-diff 安装脚本
# 使用方法: curl -sSL https://raw.githubusercontent.com/yourusername/yml-diff/main/install.sh | bash
set -e

REPO="JimDa/yml-diff"  # 替换为你的实际 GitHub 用户名/仓库名
VERSION="${1:-latest}"

# 检测操作系统和架构
detect_platform() {
    case "$(uname -s)" in
        Darwin)
            if [[ "$(uname -m)" == "arm64" ]]; then
                echo "macos-arm64"
            else
                echo "macos-x86_64"
            fi
            ;;
        Linux)
            echo "linux-x86_64"
            ;;
        MINGW* | MSYS* | CYGWIN*)
            echo "windows-x86_64.exe"
            ;;
        *)
            echo "Unsupported platform: $(uname -s)" >&2
            exit 1
            ;;
    esac
}

# 获取最新版本
get_latest_version() {
    curl -s "https://api.github.com/repos/$REPO/releases/latest" | \
        grep '"tag_name":' | \
        sed -E 's/.*"([^"]+)".*/\1/'
}

main() {
    echo "🔍 检测平台..."
    PLATFORM=$(detect_platform)
    echo "平台: $PLATFORM"

    if [[ "$VERSION" == "latest" ]]; then
        echo "🔍 获取最新版本..."
        VERSION=$(get_latest_version)
    fi
    
    echo "版本: $VERSION"
    
    BINARY_NAME="yml-diff-$PLATFORM"
    DOWNLOAD_URL="https://github.com/$REPO/releases/download/$VERSION/$BINARY_NAME"
    
    echo "⬇️  下载 $DOWNLOAD_URL"
    
    # 创建临时目录
    TEMP_DIR=$(mktemp -d)
    trap "rm -rf $TEMP_DIR" EXIT
    
    # 下载二进制文件
    if command -v curl >/dev/null 2>&1; then
        curl -L -o "$TEMP_DIR/yml-diff" "$DOWNLOAD_URL"
    elif command -v wget >/dev/null 2>&1; then
        wget -O "$TEMP_DIR/yml-diff" "$DOWNLOAD_URL"
    else
        echo "❌ 需要 curl 或 wget 来下载文件" >&2
        exit 1
    fi
    
    # 设置执行权限
    chmod +x "$TEMP_DIR/yml-diff"
    
    # 确定安装目录
    if [[ ":$PATH:" == *":$HOME/.local/bin:"* ]] && [[ -d "$HOME/.local/bin" ]]; then
        INSTALL_DIR="$HOME/.local/bin"
    elif [[ ":$PATH:" == *":$HOME/bin:"* ]] && [[ -d "$HOME/bin" ]]; then
        INSTALL_DIR="$HOME/bin"
    elif [[ -w "/usr/local/bin" ]]; then
        INSTALL_DIR="/usr/local/bin"
    else
        echo "⚠️  无法找到合适的安装目录，将安装到 $HOME/.local/bin"
        INSTALL_DIR="$HOME/.local/bin"
        mkdir -p "$INSTALL_DIR"
    fi
    
    # 安装
    echo "📦 安装到 $INSTALL_DIR"
    cp "$TEMP_DIR/yml-diff" "$INSTALL_DIR/yml-diff"
    
    echo "✅ yml-diff 安装成功!"
    echo "💡 使用方法: yml-diff --old config_v1.yml --new config_v2.yml"
    
    # 验证安装
    if command -v yml-diff >/dev/null 2>&1; then
        echo "🎉 安装验证成功!"
        yml-diff --version
    else
        echo "⚠️  请确保 $INSTALL_DIR 在你的 PATH 中"
        echo "   可以运行: export PATH=\"$INSTALL_DIR:\$PATH\""
    fi
}

main "$@"