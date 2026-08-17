# Maze Runner（走迷宫）

一款用 **Rust + [macroquad](https://github.com/not-fl3/macroquad)** 编写的 2D 走迷宫小游戏。
从左上角的绿色起点出发，穿过随机生成的迷宫，抵达右下角的金色终点即通关。

![tech](https://img.shields.io/badge/Rust-1.97+-orange) ![lib](https://img.shields.io/badge/macroquad-0.4-blue)

## 功能特性

- 🌀 **随机迷宫**：迭代式递归回溯（DFS）生成，保证任意两点间只有唯一路径（完美迷宫）
- 🎮 **流畅操作**：WASD / 方向键移动，带平滑插值动画，可按住连续走；撞墙自动停下
- 👁️ **视野迷雾**：墙体遮挡视线（射线投射），**隔墙的格子与墙壁完全漆黑不可见**；
  视野内亮度从玩家脚下平滑衰减，到视野边缘降为全黑；`F` 键持久开关，
  **按住 `Tab` 可临时移除迷雾**（松开立即恢复）
- ⏸️ **暂停菜单**：`Esc` 暂停/继续，`Q` 在暂停菜单中退出——Esc 不再直接退游戏
- ⌨️ **中文输入法友好**：启动时自动禁用 IME（输入法），WASD 等按键直达游戏，
  不会被拼音合成截获，切换窗口焦点后也不会重新弹输入法
- ⏱️ **计时与步数**：HUD 实时显示用时、移动步数、关卡与迷宫尺寸
- 💡 **路线提示**：按 `H` 显示 BFS 计算出的最短通关路线（远处自动变暗）
- 🏆 **关卡递进**：每通关一次，迷宫从 15×15 自动增大一档，最大 31×31
- 🗺️ **足迹高亮**：走过的格子地面会变亮，方便回顾
- 📐 窗口可缩放，迷宫自动适配居中

## 如何运行

需要 Rust 工具链（≥1.75，推荐最新 stable）：

```bash
cd maze_game
cargo run --release
```

> Windows 下游戏以 GUI 子系统运行（`windows_subsystem = "windows"`），
> 双击 `maze_game.exe` 或从资源管理器启动时**不会伴随黑色控制台窗口**。
> 首次编译需要下载并编译 macroquad 及其依赖，耗时约 1~3 分钟。
> Windows / macOS / Linux 均可运行。

### 离线构建说明（本机环境）

本项目当前带有 `vendor/` 目录（35 个依赖 crate，SHA256 已与 Cargo.lock 校验）
和 `.cargo/config.toml`（指向 vendored 源），因此在网络受限 / TLS 不可用的
环境（如本机沙箱）也可以完全离线构建：

```bash
cargo build --release --offline   # 或 cargo run --release --offline
```

在正常联网的机器上，可以删除 `.cargo/config.toml` 和 `vendor/` 目录，
改回从 crates.io 拉取依赖（`cargo build --release` 即可）。

需要重新 vendor 依赖时，先配置 HTTP 镜像索引并生成锁文件，再执行：

```bash
# 1) .cargo/config.toml 换成：
#    [source.crates-io]
#    replace-with = "ustc"
#    [source.ustc]
#    registry = "sparse+http://mirrors.ustc.edu.cn/crates.io-index/"
# 2) 解析依赖：
cargo generate-lockfile
# 3) 下载并校验依赖（Node.js ≥ 18）：
node tools/fetch-vendor.mjs
# 4) .cargo/config.toml 换回 vendored 源后离线构建
```

## 操作说明

| 按键 | 功能 |
| --- | --- |
| `↑ ↓ ← →` / `W A S D` | 移动（可按住连续走） |
| `Esc` | 暂停 / 继续（暂停菜单中可选退出，避免误触） |
| `R` | 重新生成当前关卡迷宫 |
| `H` | 开关最短路线提示 |
| `F` | 开关视野迷雾（持久） |
| `Tab` | 按住临时移除迷雾，松开恢复（观察全图） |
| `Q` | 在暂停菜单中退出游戏 |

> 中文输入法：游戏启动时会自动禁用 IME，无需手动切换中英文输入法。

## 项目结构

```
maze_game/
├── Cargo.toml       # 项目配置（依赖 macroquad）
├── README.md
└── src/
    ├── main.rs      # 游戏主循环：输入、状态、渲染、HUD
    └── maze.rs      # 迷宫模型：DFS 生成 + BFS 求解（含单元测试）
```

## 实现细节

- **迷宫生成**（`maze.rs`）：以栈模拟递归回溯。从起点出发，随机挑选未访问的相邻格子
  打通墙壁并压栈；无路可走时回溯，直到所有格子被访问。天然生成无环、全连通的完美迷宫。
  随机数使用自实现的 xorshift64，零额外依赖。
- **迷宫求解**（`maze.rs`）：BFS 从起点扩散，记录每个格子的前驱，从终点回溯还原最短路径。
- **视野迷雾**（`main.rs`）：从玩家插值位置向 360° 均匀发射 720 条射线，用 DDA
  连续步进（精确到下一格线），跨格时检查所跨的那面墙：有墙则记录"这面墙在距离 t
  处被射线击中"并截断，越界时记录外墙命中。格子按被照到的最短距离做幂次衰减
  并 3×3 模糊；**未被任何射线直接照到的格子严格归零**——隔墙的格子完全漆黑。
  墙段只渲染被射线实际击中的（按命中距离衰减）——射线到不了的墙（视线被其他墙
  挡住的墙背面）与背景同色，光线不会穿透或绕过墙体；视野边缘恰好降为全黑。
- **中文输入法**：调用 `miniquad::window::set_ime_enabled(false)` 禁用 IME，
  miniquad Windows 后端会记录该标志，窗口重新聚焦时也不会重新打开输入法。
- **移动**（`main.rs`）：格子制移动 + 0.12s 平滑插值动画（smoothstep 缓动），
  移动前检查目标方向墙壁，实现碰撞。
- **测试**：`cargo test` 会验证迷宫全连通性以及求解路径的真实可达性。

## 后续可扩展方向

- 迷雾模式（只能看到玩家周围一小圈）
- 道具：钥匙 / 传送门 / 限时加速
- 音效与背景音乐（macroquad 自带 audio 支持）
- 高分榜（本地存储最优用时）
- 自定义迷宫尺寸与皮肤
