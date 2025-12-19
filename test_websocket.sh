#!/bin/bash
# 完整的 WebSocket 推送测试脚本 (使用虚拟环境)
# 自动创建虚拟环境, 安装依赖, 启动 Gateway, 运行测试, 清理

set -e  # 遇到错误立即退出

echo "=========================================="
echo "WebSocket 推送功能 - 完整测试"
echo "=========================================="
echo ""

# 颜色定义
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# 清理函数
cleanup() {
    echo ""
    echo "清理进程..."
    pkill -f "zero_x_infinity.*gateway" 2>/dev/null || true
    sleep 1
    echo "✅ 清理完成"
}

# 设置 trap 确保退出时清理
trap cleanup EXIT INT TERM

# 1. 编译检查
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "1. 编译检查"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
cargo build --release 2>&1 | tail -3
if [ $? -ne 0 ]; then
    echo -e "${RED}❌ 编译失败${NC}"
    exit 1
fi
echo -e "${GREEN}✅ 编译成功${NC}"
echo ""

# 2. 设置 Python 虚拟环境
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "2. 设置 Python 环境"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

VENV_DIR=".venv_test"

if [ ! -d "$VENV_DIR" ]; then
    echo "创建虚拟环境..."
    python3 -m venv $VENV_DIR
fi

echo "激活虚拟环境..."
source $VENV_DIR/bin/activate

echo "安装 websockets..."
pip install websockets --quiet
if [ $? -ne 0 ]; then
    echo -e "${RED}❌ 无法安装 websockets${NC}"
    exit 1
fi

echo -e "${GREEN}✅ Python 环境就绪${NC}"
echo ""

# 3. 启动 Gateway
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "3. 启动 Gateway"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# 清理旧进程
pkill -f "zero_x_infinity.*gateway" 2>/dev/null || true
sleep 1

# 启动 Gateway (后台)
echo "启动命令: cargo run --release -- --gateway --port 8080"
nohup cargo run --release -- --gateway --port 8080 > /tmp/gateway_test.log 2>&1 &
GATEWAY_PID=$!
echo "Gateway PID: $GATEWAY_PID"

# 等待 Gateway 启动
echo "等待 Gateway 启动..."
sleep 5

# 检查 Gateway 是否运行
if ! ps -p $GATEWAY_PID > /dev/null 2>&1; then
    echo -e "${RED}❌ Gateway 启动失败${NC}"
    echo "查看日志:"
    tail -20 /tmp/gateway_test.log
    deactivate
    exit 1
fi

# 检查端口
if ! lsof -i:8080 > /dev/null 2>&1; then
    echo -e "${RED}❌ Gateway 未监听 8080 端口${NC}"
    echo "查看日志:"
    tail -20 /tmp/gateway_test.log
    deactivate
    exit 1
fi

echo -e "${GREEN}✅ Gateway 启动成功${NC}"
echo ""

# 显示 Gateway 日志
echo "Gateway 启动日志:"
echo "----------------------------------------"
tail -15 /tmp/gateway_test.log | grep -E "(Gateway|WebSocket|listening)" || tail -15 /tmp/gateway_test.log
echo "----------------------------------------"
echo ""

# 4. 运行 WebSocket 测试
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "4. WebSocket 功能测试"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
python test_ws_client.py
TEST_RESULT=$?

# 退出虚拟环境
deactivate

echo ""
if [ $TEST_RESULT -eq 0 ]; then
    echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${GREEN}✅ 所有测试通过!${NC}"
    echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    exit 0
else
    echo -e "${RED}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${RED}❌ 测试失败${NC}"
    echo -e "${RED}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
    echo "Gateway 日志:"
    tail -30 /tmp/gateway_test.log
    exit 1
fi
