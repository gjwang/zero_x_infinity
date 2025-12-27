#!/bin/bash
# 依赖检查和启动脚本 - 参考 test_persistence.sh

check_and_start_tdengine() {
    echo "检查 TDengine (Docker)..."
    
    # 检查 Docker 是否运行
    if ! docker info > /dev/null 2>&1; then
        echo "❌ Docker 未运行"
        echo "   启动: open -a Docker"
        return 1
    fi
    echo "✅ Docker 运行中"
    
    # 检查 TDengine 容器是否存在
    if ! docker ps -a | grep -q tdengine; then
        echo "⚠️  TDengine 容器不存在,正在创建..."
        docker run -d --name tdengine -p 6030:6030 -p 6041:6041 tdengine/tdengine:latest
        sleep 5
        echo "✅ TDengine 容器已创建"
    fi
    
    # 检查 TDengine 是否运行
    if ! docker ps | grep -q tdengine; then
        echo "⚠️  TDengine 未运行,正在启动..."
        docker start tdengine
        sleep 3
        echo "✅ TDengine 已启动"
    else
        echo "✅ TDengine 运行中"
    fi
    
    # 测试连接
    if docker exec tdengine taos -s "SELECT SERVER_VERSION();" > /dev/null 2>&1; then
        echo "✅ TDengine 连接成功"
        return 0
    else
        echo "❌ TDengine 连接失败"
        return 1
    fi
}

check_python_deps() {
    echo "检查 Python 依赖..."
    
    if ! command -v uv >/dev/null; then
        echo "❌ uv 未安装. 请运行 ./scripts/setup-dev.sh"
        return 1
    fi

    # check if uv sync needed
    if ! uv run python3 -c "import websockets" 2>/dev/null; then
        echo "⚠️  依赖缺失或未同步. 正在运行 uv sync..."
        uv sync
    fi
    
    echo "✅ Python 依赖就绪 (via uv)"
    return 0
}

check_port() {
    local port=$1
    if lsof -i:$port &> /dev/null; then
        echo "❌ 端口 $port 已被占用"
        lsof -i:$port
        return 1
    fi
    echo "✅ 端口 $port 可用"
    return 0
}

main() {
    echo "=========================================="
    echo "依赖检查和启动"
    echo "=========================================="
    echo ""
    
    local failed=0
    
    check_and_start_tdengine || failed=1
    echo ""
    
    check_python_deps || failed=1
    echo ""
    
    check_port 8080 || failed=1
    echo ""
    
    if [ $failed -eq 1 ]; then
        echo "❌ 依赖检查失败"
        return 1
    fi
    
    echo "✅ 所有依赖就绪"
    return 0
}

main "$@"
