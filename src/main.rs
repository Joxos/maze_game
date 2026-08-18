//! 迷宫游戏主程序：渲染、输入、游戏状态。
//!
//! 玩法：从左上角（绿色起点）出发，走到右下角（金色终点）。
//! 每通过一关，迷宫尺寸自动增大一档。
//!
//! 特色：
//! - 视野迷雾：墙体遮挡视线，可见区域随距离逐级变暗（F 键开关）
//! - 暂停菜单：Esc 暂停/继续，Q 在暂停时退出（Esc 不再直接退游戏）
//! - 中文输入法：启动时通过 miniquad 禁用 IME，WASD/方向键直达游戏

// Windows 下使用 GUI 子系统：双击运行不再伴随黑色控制台窗口
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod maze;
mod maze_gen;

use std::collections::HashSet;

use macroquad::prelude::*;

use maze::{opposite, Maze, E, N, S, W};
use maze_gen::Generator;

/// 顶部 HUD 区域高度
const HUD_H: f32 = 96.0;
/// 每步移动动画时长（秒）
const MOVE_TIME: f32 = 0.12;
/// 视野射线数量（360° 均分，0.25°/条）。
/// 角分辨率决定了"掠射角"墙段（与视线近乎平行的远墙）能否被命中，
/// 漏网墙段另有中点补射兜底（见 compute_vision），双保险。
const VISION_RAYS: usize = 1440;
/// 相机视口可显示的格子数（宽 x 高）：镜头跟随玩家，迷宫再大也只画视口内
const VIEW_COLS: f32 = 21.0;
const VIEW_ROWS: f32 = 19.0;

// ---- 配色 ----
const BG: Color = Color::new(0.063, 0.075, 0.122, 1.0); // 背景
const FLOOR: Color = Color::new(0.110, 0.133, 0.200, 1.0); // 未探索地面
const FLOOR_VISITED: Color = Color::new(0.157, 0.196, 0.290, 1.0); // 已探索地面
const WALL: Color = Color::new(0.81, 0.85, 0.92, 1.0); // 墙壁
const START: Color = Color::new(0.24, 0.86, 0.52, 1.0); // 起点
const GOAL: Color = Color::new(1.0, 0.82, 0.40, 1.0); // 终点
const PLAYER: Color = Color::new(0.30, 0.79, 0.94, 1.0); // 玩家
const PLAYER_EDGE: Color = Color::new(0.13, 0.44, 0.60, 1.0); // 玩家描边
const SOLUTION: Color = Color::new(0.30, 0.90, 0.60, 0.55); // 路线提示
const TEXT: Color = Color::new(0.92, 0.94, 1.0, 1.0);
const TEXT_DIM: Color = Color::new(0.55, 0.60, 0.75, 1.0);

#[derive(Clone, Copy, PartialEq)]
enum State {
    Playing,
    Paused,
    Won,
}

/// 视野计算结果：
/// - `cell_light[i]`：格子 i 的光照 0..=1（1 = 玩家脚下，0 = 完全不可见）
/// - `wall_hits[i * 4 + dir]`：墙段被射线命中的距离（INFINITY = 从未被命中，
///   即玩家看不到这面墙，完全不渲染）
struct Vision {
    cell_light: Vec<f32>,
    wall_hits: Vec<f32>,
    range: f32,
}

struct Game {
    maze: Maze,
    /// 玩家当前所在格子
    px: usize,
    py: usize,
    /// 移动动画的起点（格子坐标，浮点）
    from: (f32, f32),
    /// 移动动画进度 0..=1
    anim: f32,
    moves: u32,
    time: f32,
    state: State,
    /// 玩家走过的格子（地面高亮）
    visited: HashSet<(usize, usize)>,
    show_solution: bool,
    /// 视野迷雾开关
    fog: bool,
    /// 已通关次数（决定迷宫大小与关卡编号）
    wins: u32,
    /// 当前迷宫生成算法（G 键切换）
    gen: Generator,
    /// 生长树 newest 比例（仅 GrowingTree 生效，[ ] 键调整）
    gt_ratio: u8,
    /// 环状结构开关（L 键切换，拆墙成环后"贴墙走"不再保证通关）
    loops: bool,
    /// 当前迷宫的 BFS 最优解
    solution: Vec<(usize, usize)>,
    /// 视野计算结果（迷雾开启时每帧更新）
    vision: Vision,
    /// 插值后的玩家位置（格子坐标，含 0.5 中心偏移）
    player_pos: (f32, f32),
}

impl Game {
    fn new(wins: u32, gen: Generator, loops: bool, gt_ratio: u8) -> Self {
        // 10x10 起步，每通关一次长宽各 +5，上不封顶
        let size = 10 + 5 * wins as usize;
        let gen = gen.with_gt_ratio(gt_ratio);
        // 环数 ≈ 5% 格子数（至少 4 个），避开死胡同拆墙，支路全保留
        let loop_count = if loops {
            Some(((size * size * 5) / 100).max(4))
        } else {
            None
        };
        let maze = maze_gen::generate(size, gen, loop_count);
        let mut visited = HashSet::new();
        visited.insert((0, 0));
        let solution = maze.solve((0, 0), (size - 1, size - 1));
        let vision = full_vision(&maze);
        Game {
            maze,
            px: 0,
            py: 0,
            from: (0.0, 0.0),
            anim: 1.0,
            moves: 0,
            time: 0.0,
            state: State::Playing,
            visited,
            show_solution: false,
            fog: true,
            wins,
            gen,
            gt_ratio,
            loops,
            solution,
            vision,
            player_pos: (0.5, 0.5),
        }
    }

    fn goal(&self) -> (usize, usize) {
        (self.maze.width - 1, self.maze.height - 1)
    }
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Maze Runner - Rust + macroquad".to_owned(),
        window_width: 920,
        window_height: 940,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    // 优雅处理中文输入法：禁用 IME，按键（WASD/方向键）直接进入游戏，
    // 不再被输入法拼音合成截获；Windows 后端在窗口重新聚焦时也不会重新打开 IME。
    miniquad::window::set_ime_enabled(false);

    // 默认玩法：调研推荐的组合——生长树（60% 最新，支路/河道平衡）+ 环状化
    let mut game = Game::new(
        0,
        Generator::GrowingTree {
            newest_ratio: maze_gen::DEFAULT_GT_RATIO,
        },
        true,
        maze_gen::DEFAULT_GT_RATIO,
    );
    loop {
        update(&mut game);
        draw(&game);
        next_frame().await
    }
}

// ---------- 逻辑 ----------

fn update(g: &mut Game) {
    // Esc：播放中暂停，暂停中继续（胜利后无效）。不再绑定退出。
    if is_key_pressed(KeyCode::Escape) {
        g.state = match g.state {
            State::Playing => State::Paused,
            State::Paused => State::Playing,
            State::Won => State::Won,
        };
    }
    // R：同算法重新生成新迷宫
    if is_key_pressed(KeyCode::R) {
        *g = Game::new(g.wins, g.gen, g.loops, g.gt_ratio);
        return;
    }
    // G：切换到下一个生成算法并立即重新生成
    if is_key_pressed(KeyCode::G) {
        let next = g.gen.next();
        *g = Game::new(g.wins, next, g.loops, g.gt_ratio);
        return;
    }
    // [ ]：调整生长树 newest 比例（仅 GrowingTree 生效），±10%
    if matches!(g.gen, Generator::GrowingTree { .. }) {
        if is_key_pressed(KeyCode::LeftBracket) {
            let r = g.gt_ratio.saturating_sub(10).max(10);
            *g = Game::new(g.wins, g.gen, g.loops, r);
            return;
        }
        if is_key_pressed(KeyCode::RightBracket) {
            let r = (g.gt_ratio + 10).min(90);
            *g = Game::new(g.wins, g.gen, g.loops, r);
            return;
        }
    }
    // L：环状结构开关（拆墙成环，每拆一面墙新增一个独立环）
    if is_key_pressed(KeyCode::L) {
        *g = Game::new(g.wins, g.gen, !g.loops, g.gt_ratio);
        return;
    }
    if is_key_pressed(KeyCode::F) {
        g.fog = !g.fog;
    }
    if is_key_pressed(KeyCode::H) {
        g.show_solution = !g.show_solution;
    }

    match g.state {
        State::Paused => {
            // 暂停菜单里才能退出（避免误触）
            if is_key_pressed(KeyCode::Q) {
                std::process::exit(0);
            }
            return;
        }
        State::Won => return,
        State::Playing => {}
    }

    let dt = get_frame_time();
    g.time += dt;

    // 推进移动动画
    if g.anim < 1.0 {
        g.anim = (g.anim + dt / MOVE_TIME).min(1.0);
    }

    // 动画结束后可以开始新的一步
    if g.anim >= 1.0 {
        if let Some((dx, dy)) = held_dir() {
            let nx = g.px as i32 + dx;
            let ny = g.py as i32 + dy;
            if nx >= 0 && ny >= 0 && (nx as usize) < g.maze.width && (ny as usize) < g.maze.height {
                let dir = dir_wall(dx, dy);
                if !g.maze.wall(g.px, g.py, dir) {
                    g.from = (g.px as f32, g.py as f32);
                    g.px = nx as usize;
                    g.py = ny as usize;
                    g.anim = 0.0;
                    g.moves += 1;
                    g.visited.insert((g.px, g.py));
                }
            }
        }
    }

    // 到达终点即通关（等动画走完再判定，避免提前弹遮罩）
    if g.anim >= 1.0 && (g.px, g.py) == g.goal() {
        g.state = State::Won;
        g.wins += 1;
    }

    // 记录插值位置并更新视野光照。
    // F 为持久开关；按住 Tab 临时移除迷雾（松开立即恢复）。
    let t = smoothstep(g.anim);
    g.player_pos = (
        g.from.0 + (g.px as f32 - g.from.0) * t + 0.5,
        g.from.1 + (g.py as f32 - g.from.1) * t + 0.5,
    );
    let fog_active = g.fog && !is_key_down(KeyCode::Tab);
    if fog_active {
        g.vision = compute_vision(&g.maze, g.player_pos.0, g.player_pos.1);
    } else {
        g.vision = full_vision(&g.maze);
    }
}

/// 当前按下的移动方向（WASD / 方向键）
fn held_dir() -> Option<(i32, i32)> {
    if is_key_down(KeyCode::Up) || is_key_down(KeyCode::W) {
        return Some((0, -1));
    }
    if is_key_down(KeyCode::Down) || is_key_down(KeyCode::S) {
        return Some((0, 1));
    }
    if is_key_down(KeyCode::Left) || is_key_down(KeyCode::A) {
        return Some((-1, 0));
    }
    if is_key_down(KeyCode::Right) || is_key_down(KeyCode::D) {
        return Some((1, 0));
    }
    None
}

fn dir_wall(dx: i32, dy: i32) -> usize {
    match (dx, dy) {
        (0, -1) => N,
        (1, 0) => E,
        (0, 1) => S,
        (-1, 0) => W,
        _ => unreachable!(),
    }
}

/// 视野半径（格子数）：随迷宫尺寸增长，封顶 12 格
fn vision_range(w: usize, h: usize) -> f32 {
    ((w as f32).min(h as f32) * 0.5).clamp(5.0, 12.0)
}

/// 无迷雾视野：所有格子满光、所有墙段可见（迷雾关闭或按住 Tab 窥视时使用）
fn full_vision(maze: &Maze) -> Vision {
    Vision {
        cell_light: vec![1.0; maze.width * maze.height],
        wall_hits: vec![0.0; maze.width * maze.height * 4],
        range: vision_range(maze.width, maze.height),
    }
}

/// 距离 d 处的光照值（0..=1，幂次曲线平滑衰减，视野边缘恰好为 0）
fn light_at(d: f32, range: f32) -> f32 {
    let tt = (d / range).clamp(0.0, 1.0);
    (1.0 - tt).powf(1.6)
}

/// 从 (ox,oy) 向墙段中点 (mx,my) 发射单条精确射线（DDA 步进），
/// 目标是 (wx,wy) 格 dir 方向的墙段。
/// - 先跨过其他墙 → None（被遮挡，不可见）
/// - 跨过目标墙段 → Some(命中距离)
/// 用于兜底角分辨率漏掉的"掠射角"远墙：中点射线精确指向墙段，
/// 不存在角度采样间隙，且与扇形射线一样遵守墙体遮挡（不产生泄漏）。
fn ray_to(
    maze: &Maze,
    ox: f32,
    oy: f32,
    mx: f32,
    my: f32,
    wx: usize,
    wy: usize,
    wdir: usize,
) -> Option<f32> {
    let (vx, vy) = (mx - ox, my - oy);
    let total = (vx * vx + vy * vy).sqrt();
    if total < 1e-6 {
        return None;
    }
    let (dx, dy) = (vx / total, vy / total);
    let w = maze.width as i32;
    let h = maze.height as i32;
    let (mut x, mut y) = (ox, oy);
    let (mut cx, mut cy) = (x.floor() as i32, y.floor() as i32);
    let mut t = 0.0;

    loop {
        if cx < 0 || cy < 0 || cx >= w || cy >= h {
            return None;
        }
        let tx = if dx > 0.0 {
            (cx as f32 + 1.0 - x) / dx
        } else if dx < 0.0 {
            (cx as f32 - x) / dx
        } else {
            f32::INFINITY
        };
        let ty = if dy > 0.0 {
            (cy as f32 + 1.0 - y) / dy
        } else if dy < 0.0 {
            (cy as f32 - y) / dy
        } else {
            f32::INFINITY
        };

        let (dt, dir) = if tx <= ty {
            (tx, if dx > 0.0 { E } else { W })
        } else {
            (ty, if dy > 0.0 { S } else { N })
        };
        if !(dt > 0.0) || t + dt > total + 1e-4 {
            return None; // 越过目标点仍未跨过目标墙段
        }

        let (nx, ny) = match dir {
            E => (cx + 1, cy),
            W => (cx - 1, cy),
            S => (cx, cy + 1),
            N => (cx, cy - 1),
            _ => unreachable!(),
        };

        // 跨过的边是否就是目标墙段（两侧方向都算同一面物理墙）
        let is_target = (cx as usize == wx && cy as usize == wy && dir == wdir)
            || (nx as usize == wx && ny as usize == wy && dir == opposite(wdir));
        if is_target {
            return Some(t + dt);
        }

        if nx < 0 || ny < 0 || nx >= w || ny >= h {
            return None; // 撞到其他外墙
        }
        if maze.wall(cx as usize, cy as usize, dir) {
            return None; // 撞到其他墙，被遮挡
        }

        x += dx * dt;
        y += dy * dt;
        t += dt;
        match dir {
            E => cx += 1,
            W => cx -= 1,
            S => cy += 1,
            N => cy -= 1,
            _ => unreachable!(),
        }
    }
}

/// 视野计算：从玩家位置向 360° 均匀发射射线，用 DDA 连续步进
/// （精确到下一格线），逐格漫游：
/// - 跨格时检查所跨的那面墙（精确到墙段）：有墙则记录"这面墙在距离 t
///   处被射线击中"并截断；越出迷宫边界时记录外墙命中。
/// - 格子：记录被照到的最短距离，衰减为光照后 3x3 模糊；
///   从未被任何射线直接照到的格子严格归零（隔墙全黑）。
/// - 墙段：只渲染被射线实际击中的墙（按命中距离衰减）——射线到不了的
///   墙（视线被其他墙挡住）完全不可见，光不会穿透或绕过墙。
fn compute_vision(maze: &Maze, ox: f32, oy: f32) -> Vision {
    let w = maze.width;
    let h = maze.height;
    let range = vision_range(w, h);
    let mut dist = vec![f32::INFINITY; w * h];
    let mut wall_hits = vec![f32::INFINITY; w * h * 4];

    for i in 0..VISION_RAYS {
        let a = i as f32 / VISION_RAYS as f32 * std::f32::consts::TAU;
        let (dx, dy) = (a.cos(), a.sin());
        let (mut x, mut y) = (ox, oy);
        let (mut cx, mut cy) = (x.floor() as i32, y.floor() as i32);
        let mut t = 0.0;

        loop {
            if cx < 0 || cy < 0 || cx >= w as i32 || cy >= h as i32 {
                break;
            }
            let idx = (cx as usize) + (cy as usize) * w;
            if t < dist[idx] {
                dist[idx] = t;
            }

            // 到本格横/竖边界的距离（沿射线方向）
            let tx = if dx > 0.0 {
                (cx as f32 + 1.0 - x) / dx
            } else if dx < 0.0 {
                (cx as f32 - x) / dx
            } else {
                f32::INFINITY
            };
            let ty = if dy > 0.0 {
                (cy as f32 + 1.0 - y) / dy
            } else if dy < 0.0 {
                (cy as f32 - y) / dy
            } else {
                f32::INFINITY
            };

            // 先跨过哪条边，就检查哪面墙（精确墙段）
            let (dt, dir) = if tx <= ty {
                (tx, if dx > 0.0 { E } else { W })
            } else {
                (ty, if dy > 0.0 { S } else { N })
            };
            if !(dt > 0.0) || t + dt > range {
                break; // 无前进方向或超出视野半径
            }

            let (nx, ny) = match dir {
                E => (cx + 1, cy),
                W => (cx - 1, cy),
                S => (cx, cy + 1),
                N => (cx, cy - 1),
                _ => unreachable!(),
            };
            let wi = ((cx as usize) + (cy as usize) * w) * 4 + dir;

            if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                // 越界 = 撞上外墙，记录外墙命中
                if t + dt < wall_hits[wi] {
                    wall_hits[wi] = t + dt;
                }
                break;
            }
            if maze.wall(cx as usize, cy as usize, dir) {
                // 这面墙被射线击中，记录命中距离并截断
                if t + dt < wall_hits[wi] {
                    wall_hits[wi] = t + dt;
                }
                break;
            }

            // 通过：前进到该边（格子坐标用算术推进，避免浮点误差）
            x += dx * dt;
            y += dy * dt;
            t += dt;
            match dir {
                E => cx += 1,
                W => cx -= 1,
                S => cy += 1,
                N => cy -= 1,
                _ => unreachable!(),
            }
        }
    }

    // 距离衰减 -> 光照（从玩家脚下 1.0 平滑降到视野边缘 0.0）
    let mut light = vec![0.0f32; w * h];
    for i in 0..light.len() {
        let d = dist[i];
        if d.is_finite() {
            light[i] = light_at(d, range);
        }
    }

    // 3x3 模糊，让光照过渡平滑；从未被直接照到的格子严格归零（隔墙全黑）
    let mut cell_light = vec![0.0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let mut acc = 0.0;
            let mut n = 0.0;
            for yy in -1i32..=1 {
                for xx in -1i32..=1 {
                    let nx = x as i32 + xx;
                    let ny = y as i32 + yy;
                    if nx >= 0 && ny >= 0 && nx < w as i32 && ny < h as i32 {
                        acc += light[nx as usize + ny as usize * w];
                        n += 1.0;
                    }
                }
            }
            let idx = x + y * w;
            cell_light[idx] = if dist[idx].is_finite() {
                acc / n
            } else {
                0.0
            };
        }
    }

    // 中点补射兜底：扇形射线的角分辨率在"掠射角"（与视线近乎平行的远墙）
    // 下会漏掉整面墙——对每个"邻接被照亮格子但未被任何射线命中"的墙段，
    // 从玩家向墙段中点发射一条精确射线。中点射线不存在角度间隙，
    // 且同样遵守墙体遮挡（被挡住的墙段依然不可见，不产生泄漏）。
    for y in 0..h {
        for x in 0..w {
            for dir in [N, E, S, W] {
                let wi = (x + y * w) * 4 + dir;
                if wall_hits[wi].is_finite() {
                    continue; // 已被扇形射线命中
                }
                let (nx, ny) = match dir {
                    N => (x, y.wrapping_sub(1)),
                    S => (x, y + 1),
                    W => (x.wrapping_sub(1), y),
                    E => (x + 1, y),
                    _ => unreachable!(),
                };
                let lit_here = cell_light[x + y * w] > 0.0;
                let lit_there = nx < w && ny < h && cell_light[nx + ny * w] > 0.0;
                if !lit_here && !lit_there {
                    continue; // 两侧都不亮：不可见，无需补射
                }
                // 墙段中点
                let (mx, my) = match dir {
                    N => (x as f32 + 0.5, y as f32),
                    S => (x as f32 + 0.5, y as f32 + 1.0),
                    W => (x as f32, y as f32 + 0.5),
                    E => (x as f32 + 1.0, y as f32 + 0.5),
                    _ => unreachable!(),
                };
                if let Some(d) = ray_to(maze, ox, oy, mx, my, x, y, dir) {
                    if d <= range && d < wall_hits[wi] {
                        wall_hits[wi] = d;
                    }
                }
            }
        }
    }

    Vision {
        cell_light,
        wall_hits,
        range,
    }
}

// ---------- 渲染 ----------

/// 相机布局：格子像素尺寸 + 相机中心（格子坐标）。
/// 相机锁定在玩家插值位置（移动动画内也平滑跟随），并夹取在迷宫边界内；
/// 迷宫小于视口时居中显示。
fn layout(g: &Game) -> (f32, f32, f32) {
    let sw = screen_width();
    let sh = screen_height();
    let avail_w = sw - 24.0;
    let avail_h = sh - HUD_H - 24.0;
    let cell = (avail_w / VIEW_COLS)
        .min(avail_h / VIEW_ROWS)
        .floor()
        .max(20.0);
    let (px, py) = g.player_pos;
    let half_w = avail_w / (2.0 * cell); // 视口半宽（格子数）
    let half_h = avail_h / (2.0 * cell);
    let w = g.maze.width as f32;
    let h = g.maze.height as f32;
    let cam_x = if w <= 2.0 * half_w {
        w / 2.0 // 迷宫比视口小：居中
    } else {
        px.clamp(half_w, w - half_w)
    };
    let cam_y = if h <= 2.0 * half_h {
        h / 2.0
    } else {
        py.clamp(half_h, h - half_h)
    };
    (cell, cam_x, cam_y)
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// 颜色线性插值（t=0 得 a，t=1 得 b）
fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color::new(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        a.a + (b.a - a.a) * t,
    )
}

fn draw(g: &Game) {
    clear_background(BG);
    let (cell, cam_x, cam_y) = layout(g);
    let sw = screen_width();
    let sh = screen_height();
    let avail_w = sw - 24.0;
    let avail_h = sh - HUD_H - 24.0;
    let view_cx = 12.0 + avail_w / 2.0;
    let view_cy = HUD_H + avail_h / 2.0;
    // 世界坐标 -> 屏幕坐标：相机中心（cam_x, cam_y）落在视口中心
    let ox = view_cx - cam_x * cell;
    let oy = view_cy - cam_y * cell;
    let w = g.maze.width;
    let h = g.maze.height;

    // 视口内的可见格子范围（大迷宫只绘制这一部分）
    let half_cols = (avail_w / (2.0 * cell)).ceil() as i32 + 1;
    let half_rows = (avail_h / (2.0 * cell)).ceil() as i32 + 1;
    let x0 = ((cam_x - half_cols as f32).floor().max(0.0)) as usize;
    let x1 = ((cam_x + half_cols as f32).ceil() as usize).min(w);
    let y0 = ((cam_y - half_rows as f32).floor().max(0.0)) as usize;
    let y1 = ((cam_y + half_rows as f32).ceil() as usize).min(h);

    // 地面（按视野光照混合：隔墙格子完全漆黑，视野内随距离渐暗至全黑）
    for y in y0..y1 {
        for x in x0..x1 {
            let base = if g.visited.contains(&(x, y)) {
                FLOOR_VISITED
            } else {
                FLOOR
            };
            let dark = 1.0 - g.vision.cell_light[x + y * w];
            let c = if dark > 0.001 {
                lerp_color(base, BG, dark)
            } else {
                base
            };
            draw_rectangle(
                ox + x as f32 * cell + 1.5,
                oy + y as f32 * cell + 1.5,
                cell - 3.0,
                cell - 3.0,
                c,
            );
        }
    }

    // 通关路线提示（H 键切换，随所在格光照变暗；视口外的线段由 GL 裁剪）
    if g.show_solution {
        for pair in g.solution.windows(2) {
            let (x1, y1) = pair[0];
            let (x2, y2) = pair[1];
            let light = g.vision.cell_light[x1 + y1 * w].min(g.vision.cell_light[x2 + y2 * w]);
            let c = lerp_color(SOLUTION, BG, 1.0 - light);
            let ax = ox + (x1 as f32 + 0.5) * cell;
            let ay = oy + (y1 as f32 + 0.5) * cell;
            let bx = ox + (x2 as f32 + 0.5) * cell;
            let by = oy + (y2 as f32 + 0.5) * cell;
            draw_line(ax, ay, bx, by, 4.0, c);
        }
    }

    // 墙壁（两侧任一格被照亮即可见，取较亮一侧：眼前的墙是视野剪影边界；
    // 两侧都黑的墙完全不可见）
    for y in y0..y1 {
        for x in x0..x1 {
            let fx = ox + x as f32 * cell;
            let fy = oy + y as f32 * cell;
            if g.maze.wall(x, y, N) {
                draw_line(fx, fy, fx + cell, fy, 3.0, wall_color(g, x, y, N));
            }
            if g.maze.wall(x, y, S) {
                draw_line(fx, fy + cell, fx + cell, fy + cell, 3.0, wall_color(g, x, y, S));
            }
            if g.maze.wall(x, y, W) {
                draw_line(fx, fy, fx, fy + cell, 3.0, wall_color(g, x, y, W));
            }
            if g.maze.wall(x, y, E) {
                draw_line(fx + cell, fy, fx + cell, fy + cell, 3.0, wall_color(g, x, y, E));
            }
        }
    }

    // 起点（进入视野才可见；视口外不绘制）
    if x0 == 0 && y0 == 0 {
        let sl = g.vision.cell_light[0];
        let sc = if sl > 0.02 {
            lerp_color(START, BG, 1.0 - sl)
        } else {
            BG
        };
        draw_circle(ox + 0.5 * cell, oy + 0.5 * cell, cell * 0.22, sc);
    }

    // 终点（进入视野才发光，隔墙时完全不可见；视口外不绘制）
    let (gx, gy) = g.goal();
    if gx >= x0 && gx < x1 && gy >= y0 && gy < y1 {
        let gl = g.vision.cell_light[gx + gy * w];
        let pulse = 1.0 + 0.12 * (get_time() as f32 * 4.0).sin() * gl;
        let gc = if gl > 0.02 {
            lerp_color(GOAL, BG, 1.0 - gl)
        } else {
            BG
        };
        draw_circle(
            ox + (gx as f32 + 0.5) * cell,
            oy + (gy as f32 + 0.5) * cell,
            cell * 0.30 * pulse,
            gc,
        );
    }

    // 玩家（带插值动画与柔和光晕）
    let px = ox + g.player_pos.0 * cell;
    let py = oy + g.player_pos.1 * cell;
    let r = cell * 0.32;
    draw_circle(px, py, r + 6.0, Color::new(0.30, 0.79, 0.94, 0.16));
    draw_circle(px, py, r + 2.5, PLAYER_EDGE);
    draw_circle(px, py, r, PLAYER);

    draw_hud(g);
    if g.state == State::Won {
        draw_win(g);
    }
    if g.state == State::Paused {
        draw_pause(g);
    }
}

/// 墙壁颜色：只渲染被射线实际击中的墙段（玩家看得到的那一面），
/// 按命中距离衰减变暗；从未被击中的墙（视线被其他墙挡住的墙背面）
/// 与背景完全一致，不可见——光线不会"穿透"或"绕过"墙。
fn wall_color(g: &Game, x: usize, y: usize, dir: usize) -> Color {
    let w = g.maze.width;
    let d = g.vision.wall_hits[(x + y * w) * 4 + dir];
    if !d.is_finite() {
        return BG; // 这面墙从未被视线击中，完全不可见
    }
    lerp_color(WALL, BG, 1.0 - light_at(d, g.vision.range))
}

fn draw_hud(g: &Game) {
    let sw = screen_width();

    draw_text(&format!("Time {:.1}s", g.time), 16.0, 32.0, 28.0, TEXT);
    draw_text(&format!("Moves {}", g.moves), 16.0, 60.0, 22.0, TEXT_DIM);

    let title = format!(
        "Level {}  -  Maze {}x{}  -  {}",
        g.wins + 1,
        g.maze.width,
        g.maze.height,
        g.gen.label()
    );
    let tw = measure_text(&title, None, 26, 1.0).width;
    draw_text(&title, (sw - tw) / 2.0, 36.0, 26.0, TEXT);

    let hint1 = format!(
        "Arrows/WASD move    R new maze    G algorithm    L loops:{}",
        if g.loops { "On" } else { "Off" }
    );
    let h1w = measure_text(&hint1, None, 18, 1.0).width;
    draw_text(&hint1, (sw - h1w) / 2.0, 60.0, 18.0, TEXT_DIM);

    let hint2 = "H solution    F fog    [ ] tree ratio    Tab peek    Esc pause";
    let h2w = measure_text(hint2, None, 18, 1.0).width;
    draw_text(hint2, (sw - h2w) / 2.0, 84.0, 18.0, TEXT_DIM);
}

fn draw_win(g: &Game) {
    let sw = screen_width();
    let sh = screen_height();

    draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.62));

    let title = "YOU ESCAPED!";
    let tw = measure_text(title, None, 56, 1.0).width;
    draw_text(title, (sw - tw) / 2.0, sh * 0.40, 56.0, GOAL);

    let info = format!(
        "Time {:.1}s    Moves {}    Maze {}x{}",
        g.time,
        g.moves,
        g.maze.width,
        g.maze.height
    );
    let iw = measure_text(&info, None, 26, 1.0).width;
    draw_text(&info, (sw - iw) / 2.0, sh * 0.40 + 56.0, 26.0, TEXT);

    let again = "Press  R  to level up - the next maze is bigger!";
    let aw = measure_text(again, None, 22, 1.0).width;
    draw_text(&again, (sw - aw) / 2.0, sh * 0.40 + 100.0, 22.0, TEXT_DIM);
}

fn draw_pause(g: &Game) {
    let sw = screen_width();
    let sh = screen_height();

    draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.55));

    let title = "PAUSED";
    let tw = measure_text(title, None, 56, 1.0).width;
    draw_text(title, (sw - tw) / 2.0, sh * 0.34, 56.0, TEXT);

    let info = format!(
        "Generator: {}    Loops: {}",
        g.gen.label(),
        if g.loops { "On" } else { "Off" }
    );
    let iw = measure_text(&info, None, 24, 1.0).width;
    draw_text(&info, (sw - iw) / 2.0, sh * 0.34 + 52.0, 24.0, TEXT_DIM);

    let stats = format!(
        "Cycles: {}    Dead ends: {}",
        maze_gen::cycle_count(&g.maze),
        maze_gen::dead_end_count(&g.maze)
    );
    let sw2 = measure_text(&stats, None, 22, 1.0).width;
    draw_text(&stats, (sw - sw2) / 2.0, sh * 0.34 + 82.0, 22.0, TEXT_DIM);

    let opts = ["Esc - Resume", "R - New Maze", "G - Next generator", "Q - Quit"];
    for (i, line) in opts.iter().enumerate() {
        let lw = measure_text(line, None, 26, 1.0).width;
        draw_text(
            line,
            (sw - lw) / 2.0,
            sh * 0.34 + 124.0 + i as f32 * 44.0,
            26.0,
            if i == 0 { TEXT } else { TEXT_DIM },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use maze::test_maze;

    /// 墙段索引：(x + y * w) * 4 + dir
    fn wi(x: usize, y: usize, dir: usize, w: usize) -> usize {
        (x + y * w) * 4 + dir
    }

    /// 场景 1：5x5 迷宫，只有第 2 行是一条东西向走廊，其余全部封死。
    /// 玩家在走廊西端 (0.5, 2.5)。走廊应照亮，走廊外所有格子必须严格全黑，
    /// 且所有不邻接走廊的墙段（从未被射线命中）必须不可见。
    #[test]
    fn vision_does_not_leak_through_walls() {
        let w = 5;
        let h = 5;
        let maze = test_maze(w, h, &[(0, 2, E), (1, 2, E), (2, 2, E), (3, 2, E)]);
        let vision = compute_vision(&maze, 0.5, 2.5);

        // 走廊第 2 行全部照亮
        for x in 0..w {
            assert!(
                vision.cell_light[x + 2 * w] > 0.02,
                "corridor cell ({},{}) should be lit, got {}",
                x,
                2,
                vision.cell_light[x + 2 * w]
            );
        }
        // 走廊外的所有格子严格全黑
        for y in 0..h {
            for x in 0..w {
                if y == 2 {
                    continue;
                }
                assert_eq!(
                    vision.cell_light[x + y * w],
                    0.0,
                    "cell ({},{}) behind wall leaks light",
                    x,
                    y
                );
            }
        }
        // 玩家能看到的墙段（走廊边界、走廊尽头）必须被射线命中
        for x in 0..4 {
            assert!(vision.wall_hits[wi(x, 2, N, w)].is_finite(), "corridor N wall ({},2) should be visible", x);
            assert!(vision.wall_hits[wi(x, 2, S, w)].is_finite(), "corridor S wall ({},2) should be visible", x);
        }
        assert!(vision.wall_hits[wi(4, 2, E, w)].is_finite(), "corridor end wall should be visible");
        // 深处（不邻接任何被照亮格子）的墙段必须完全不可见
        for &(x, y, dir) in &[(1, 1, E), (2, 0, E), (2, 4, W), (0, 3, S), (1, 3, E), (3, 4, N)] {
            assert!(
                !vision.wall_hits[wi(x, y, dir, w)].is_finite(),
                "wall ({},{},{}) behind walls should be invisible",
                x,
                y,
                dir
            );
        }
    }

    /// 场景 2（旧版真实泄漏回归）：玩家在 (0,0)，东墙 W1 封死；
    /// 光只能经 (0,0) 南侧门洞向下、再向东绕进 (1,1)。墙 W2 = (1,0) 与
    /// (1,1) 之间的墙，其"亮面"朝被照亮的 (1,1)——旧规则会把它渲染成
    /// 亮线（光线绕过 W1 透出），新规则下任何射线都到不了 W2，必须不可见。
    #[test]
    fn vision_wall_far_side_is_invisible() {
        let w = 5;
        let h = 5;
        let maze = test_maze(w, h, &[(0, 0, S), (0, 1, E)]);
        let vision = compute_vision(&maze, 0.5, 0.5);

        // (1,0)、(2,0) 被 W1 完全封死，必须严格全黑
        assert_eq!(vision.cell_light[1 + 0 * w], 0.0, "sealed cell (1,0) leaks light");
        assert_eq!(vision.cell_light[2 + 0 * w], 0.0, "sealed cell (2,0) leaks light");
        // 门洞下方 (0,1) 与拐角 (1,1) 被照亮（光拐弯后只能到达这里）
        assert!(vision.cell_light[0 + 1 * w] > 0.02, "cell (0,1) should be lit");
        assert!(vision.cell_light[1 + 1 * w] > 0.02, "corner cell (1,1) should be lit");
        // 光拐不过直角：更远的 (2,1) 必须全黑
        assert_eq!(vision.cell_light[2 + 1 * w], 0.0, "cell (2,1) behind the bend leaks light");
        // 玩家正前方的墙 W1 可见（它挡在眼前）
        assert!(vision.wall_hits[wi(0, 0, E, w)].is_finite(), "wall W1 in front of player should be visible");
        // 玩家上方的外墙与左侧外墙可见
        assert!(vision.wall_hits[wi(0, 0, N, w)].is_finite(), "top border wall should be visible");
        assert!(vision.wall_hits[wi(0, 0, W, w)].is_finite(), "left border wall should be visible");
        // W2 的亮面朝被照亮的 (1,1)，但玩家视线被 W1 挡住——必须不可见（核心回归）
        assert!(
            !vision.wall_hits[wi(1, 0, S, w)].is_finite(),
            "wall W2 far side leaks through W1"
        );
        assert!(!vision.wall_hits[wi(2, 0, S, w)].is_finite(), "wall (2,0,S) far side leaks");
    }

    /// 场景 3（掠射角漏墙回归）：25x25 迷宫只有第 12 行是一条 25 格长的直走廊，
    /// 玩家在走廊西端。走廊侧墙与视线近乎平行，是扇形射线角分辨率最容易
    /// 漏掉的"掠射角"远墙——旧实现（720 条射线）下 9、10 格外侧墙的角宽
    /// 不足 0.4°，整面墙落在两条射线之间、从未被命中而消失。
    /// 修复后（更密射线 + 中点补射兜底）这些墙必须可见。
    #[test]
    fn vision_glancing_angle_walls_are_visible() {
        let w = 25;
        let h = 25;
        let mut open = Vec::new();
        for x in 0..w - 1 {
            open.push((x, 12, E));
        }
        let maze = test_maze(w, h, &open);
        let vision = compute_vision(&maze, 0.5, 12.5);

        // 走廊侧墙（掠射角）在远处必须可见
        for x in 8..=11 {
            assert!(
                vision.wall_hits[wi(x, 12, S, w)].is_finite(),
                "far corridor side wall ({},12,S) missing at glancing angle",
                x
            );
        }
        // 超出视野半径（12 格）的侧墙按设计不可见（迷雾边缘）
        assert!(
            !vision.wall_hits[wi(12, 12, S, w)].is_finite(),
            "wall beyond fog range should stay invisible"
        );
        // 走廊格子被照亮
        assert!(vision.cell_light[10 + 12 * w] > 0.01, "corridor cell (10,12) should be lit");
        // 深处暗格依然严格全黑、深处墙不可见
        assert_eq!(vision.cell_light[5 + 5 * w], 0.0, "dark cell (5,5) leaks light");
        assert!(
            !vision.wall_hits[wi(5, 5, E, w)].is_finite(),
            "deep wall should stay invisible"
        );
    }
}
