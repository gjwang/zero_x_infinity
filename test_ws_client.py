#!/usr/bin/env python3
"""
完整的 WebSocket 推送测试脚本
测试 WebSocket 连接、消息格式、推送功能
"""

import asyncio
import websockets
import json
import sys
import time
from typing import Optional

class WebSocketTester:
    def __init__(self, uri: str):
        self.uri = uri
        self.websocket: Optional[websockets.WebSocketClientProtocol] = None
        self.received_messages = []
        
    async def connect(self) -> bool:
        """测试 WebSocket 连接"""
        try:
            print(f"[TEST] 连接到: {self.uri}")
            self.websocket = await asyncio.wait_for(
                websockets.connect(self.uri),
                timeout=5.0
            )
            print("✅ WebSocket 连接成功")
            return True
        except Exception as e:
            print(f"❌ 连接失败: {e}")
            return False
    
    async def test_connected_message(self) -> bool:
        """测试 connected 消息"""
        try:
            print("\n[TEST] 等待 connected 消息...")
            message = await asyncio.wait_for(
                self.websocket.recv(),
                timeout=5.0
            )
            print(f"收到: {message}")
            
            data = json.loads(message)
            if data.get("type") == "connected" and data.get("user_id") == 1001:
                print("✅ Connected 消息格式正确")
                self.received_messages.append(data)
                return True
            else:
                print(f"❌ Connected 消息格式错误: {data}")
                return False
        except asyncio.TimeoutError:
            print("❌ 超时: 未收到 connected 消息")
            return False
        except Exception as e:
            print(f"❌ 错误: {e}")
            return False
    
    async def test_ping_pong(self) -> bool:
        """测试 ping/pong"""
        try:
            print("\n[TEST] 测试 Ping/Pong...")
            ping_msg = json.dumps({"type": "ping"})
            await self.websocket.send(ping_msg)
            print(f"发送: {ping_msg}")
            
            pong = await asyncio.wait_for(
                self.websocket.recv(),
                timeout=5.0
            )
            print(f"收到: {pong}")
            
            pong_data = json.loads(pong)
            if pong_data.get("type") == "pong":
                print("✅ Ping/Pong 正常")
                return True
            else:
                print(f"❌ Pong 消息错误: {pong_data}")
                return False
        except Exception as e:
            print(f"❌ Ping/Pong 测试失败: {e}")
            return False
    
    async def wait_for_push_events(self, timeout: float = 10.0) -> bool:
        """等待推送事件 (可选测试)"""
        try:
            print(f"\n[TEST] 等待推送事件 (超时: {timeout}s)...")
            print("提示: 如果没有交易发生,此测试会超时,这是正常的")
            
            end_time = time.time() + timeout
            while time.time() < end_time:
                try:
                    message = await asyncio.wait_for(
                        self.websocket.recv(),
                        timeout=1.0
                    )
                    data = json.loads(message)
                    print(f"收到推送: {data}")
                    self.received_messages.append(data)
                    
                    # 验证消息格式
                    if data.get("type") in ["order_update", "trade", "balance_update"]:
                        print(f"✅ 收到有效推送事件: {data.get('type')}")
                except asyncio.TimeoutError:
                    continue
            
            print(f"⚠️  未收到推送事件 (正常,因为没有交易)")
            return True  # 不强制要求有推送
        except Exception as e:
            print(f"⚠️  推送事件测试异常: {e}")
            return True  # 不强制要求
    
    async def close(self):
        """关闭连接"""
        if self.websocket:
            await self.websocket.close()
            print("\n[INFO] WebSocket 连接已关闭")
    
    async def run_all_tests(self) -> bool:
        """运行所有测试"""
        print("=" * 60)
        print("WebSocket 推送功能测试")
        print("=" * 60)
        
        # 1. 连接测试
        if not await self.connect():
            return False
        
        # 2. Connected 消息测试
        if not await self.test_connected_message():
            await self.close()
            return False
        
        # 3. Ping/Pong 测试
        if not await self.test_ping_pong():
            await self.close()
            return False
        
        # 4. 推送事件测试 (可选)
        await self.wait_for_push_events(timeout=3.0)
        
        await self.close()
        
        print("\n" + "=" * 60)
        print("✅ 所有必需测试通过!")
        print("=" * 60)
        print(f"\n总共收到 {len(self.received_messages)} 条消息")
        return True

async def main():
    uri = "ws://localhost:8080/ws?user_id=1001"
    tester = WebSocketTester(uri)
    
    try:
        success = await tester.run_all_tests()
        return 0 if success else 1
    except KeyboardInterrupt:
        print("\n\n测试被中断")
        return 1
    except Exception as e:
        print(f"\n❌ 测试失败: {e}")
        import traceback
        traceback.print_exc()
        return 1

if __name__ == "__main__":
    exit_code = asyncio.run(main())
    sys.exit(exit_code)
