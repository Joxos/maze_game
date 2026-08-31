//! 迷宫生成算法库：多种生成器 + 环状化后处理 + 统计指标。
//!
//! 所有生成器都产出连通的**完美迷宫**（生成树，环路数为 0），差异在于
//! 支路密度与河道形态（31x31、200 迷宫平均，实测）：
//!
//! | 算法 | 死胡同% | 路口% | 河道因子 | 最长路径 |
//! | --- | --- | --- | --- | --- |
//! | RecursiveBacktracker（递归回溯/DFS） | 10.2 | 9.9 | 5.8 | 487 |
//! | RandomizedPrim（随机 Prim） | 32.1 | 27.3 | 1.7 | 109 |
//! | Kruskal（并查集随机合并） | 30.3 | 26.2 | 1.8 | 142 |
//! | Wilson（loop-erased 随机游走） | 29.1 | 25.5 | 1.8 | 163 |
//! | AldousBroder（随机游走） | 29.2 | 25.6 | 1.8 | 162 |
//! | GrowingTree（生长树，newest 比例可调；50% 实测） | 21.7 | 19.9 | 2.5 | 164 |
//!
//! 环状化（`carve_loops`）：在生成树上随机拆墙，每拆一面内部墙恰好新增
//! 一个独立环；`avoid_deadends=true` 时跳过与死胡同相邻的墙，支路完整
//! 保留（实测 5%/10%/15% 拆墙可加 48/96/144 个环且死胡同 100% 保留）。
//! 有环后"贴墙走"不再保证能通关。

use crate::maze::{opposite, Maze, E, N, S, W};

/// 可选生成算法
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Generator {
    /// 迭代式递归回溯（DFS）：长蜿蜒走廊、死胡同最少
    RecursiveBacktracker,
    /// 随机 Prim：死胡同最多、分叉最密、河道最短
    RandomizedPrim,
    /// Kruskal：并查集随机合并，均衡
    Kruskal,
    /// Wilson：loop-erased 随机游走，无偏生成树
    Wilson,
    /// Aldous-Broder：朴素随机游走，无偏但较慢
    AldousBroder,
    /// 生长树：按"最新/随机"比例选取活跃格，支路度可调；
    /// `newest_ratio` = 取"最新格"的百分比（0≈Prim 风格，100≈DFS 风格）
    GrowingTree { newest_ratio: u8 },
}

/// 生长树默认"取最新格"比例（%）：60% 介于 50% 实测（死胡同 21.7%、河道 2.5）
/// 与 75% 实测（死胡同 15.8%、河道 3.5）之间，偏长通道的平衡档，适合迷雾玩法。
pub const DEFAULT_GT_RATIO: u8 = 60;

impl Generator {
    /// 全部算法（循环顺序）
    pub const ALL: [Generator; 6] = [
        Generator::RecursiveBacktracker,
        Generator::RandomizedPrim,
        Generator::Kruskal,
        Generator::Wilson,
        Generator::AldousBroder,
        Generator::GrowingTree {
            newest_ratio: DEFAULT_GT_RATIO,
        },
    ];

    /// 下一个算法（循环）
    pub fn next(self) -> Generator {
        let i = Self::ALL.iter().position(|&g| g == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }

    /// 显示名
    pub fn name(self) -> &'static str {
        match self {
            Generator::RecursiveBacktracker => "Recursive Backtracker",
            Generator::RandomizedPrim => "Randomized Prim",
            Generator::Kruskal => "Kruskal",
            Generator::Wilson => "Wilson",
            Generator::AldousBroder => "Aldous-Broder",
            Generator::GrowingTree { .. } => "Growing Tree",
        }
    }

    /// 带生长树比例的完整显示名（HUD / 暂停菜单用）
    pub fn label(self) -> String {
        match self {
            Generator::GrowingTree { newest_ratio } => {
                format!("Growing Tree ({}%)", newest_ratio)
            }
            other => other.name().to_string(),
        }
    }

    /// 应用生长树比例（非生长树算法原样返回）
    pub fn with_gt_ratio(self, ratio: u8) -> Generator {
        match self {
            Generator::GrowingTree { .. } => Generator::GrowingTree {
                newest_ratio: ratio,
            },
            other => other,
        }
    }
}

/// xorshift64 伪随机数生成器（避免额外依赖）
pub(crate) struct XorShift(u64);

impl XorShift {
    /// 固定种子（测试可复现）
    pub(crate) fn seeded(seed: u64) -> Self {
        XorShift(seed | 1)
    }

    /// 用当前系统时间（纳秒）做种子
    pub(crate) fn from_time() -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        Self::seeded(seed)
    }

    pub(crate) fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

/// 生成 size x size 的迷宫。
/// - `gen`：生成算法
/// - `loops`：`Some(n)` 时在生成后拆掉 n 面内部墙成环（避开死胡同，
///   支路全保留；每拆一面墙新增一个独立环）
pub fn generate(size: usize, gen: Generator, loops: Option<usize>) -> Maze {
    generate_seeded(size, gen, loops, XorShift::from_time().0)
}

/// 固定种子生成（测试用）
pub fn generate_seeded(size: usize, gen: Generator, loops: Option<usize>, seed: u64) -> Maze {
    let mut maze = Maze {
        width: size,
        height: size,
        walls: vec![[true; 4]; size * size],
    };
    let mut rng = XorShift::seeded(seed);
    match gen {
        Generator::RecursiveBacktracker => gen_backtracker(&mut maze, &mut rng),
        Generator::RandomizedPrim => gen_prim(&mut maze, &mut rng),
        Generator::Kruskal => gen_kruskal(&mut maze, &mut rng),
        Generator::Wilson => gen_wilson(&mut maze, &mut rng),
        Generator::AldousBroder => gen_aldous_broder(&mut maze, &mut rng),
        Generator::GrowingTree { newest_ratio } => {
            gen_growing_tree(&mut maze, &mut rng, newest_ratio as u64)
        }
    }
    if let Some(count) = loops {
        carve_loops(&mut maze, &mut rng, count, true);
    }
    maze
}

// ---------- 基础工具 ----------

fn idx(x: usize, y: usize, w: usize) -> usize {
    y * w + x
}

fn in_bounds(x: usize, y: usize, w: usize, h: usize) -> bool {
    x < w && y < h
}

/// 四个方向邻居（含越界）
fn neighbor4(x: usize, y: usize) -> [(usize, usize); 4] {
    [
        (x, y.wrapping_sub(1)), // N
        (x + 1, y),             // E
        (x, y + 1),             // S
        (x.wrapping_sub(1), y), // W
    ]
}

/// 打通 (x1,y1) 与 (x2,y2) 之间的墙（双向；两者必须相邻）
fn open_between(maze: &mut Maze, x1: usize, y1: usize, x2: usize, y2: usize) {
    let w = maze.width;
    let dir = match (x2 as i32 - x1 as i32, y2 as i32 - y1 as i32) {
        (1, 0) => E,
        (-1, 0) => W,
        (0, 1) => S,
        (0, -1) => N,
        _ => unreachable!("cells not adjacent"),
    };
    maze.walls[idx(x1, y1, w)][dir] = false;
    maze.walls[idx(x2, y2, w)][opposite(dir)] = false;
}

/// 某个格子的开放边数（= 通路数）
fn open_degree(maze: &Maze, x: usize, y: usize) -> usize {
    let w = maze.width;
    let i = idx(x, y, w);
    (0..4).filter(|&d| !maze.walls[i][d]).count()
}

/// Fisher-Yates 洗牌
fn shuffle<T>(v: &mut [T], rng: &mut XorShift) {
    for i in (1..v.len()).rev() {
        let j = rng.next() as usize % (i + 1);
        v.swap(i, j);
    }
}

// ---------- 生成器 ----------

/// 迭代式递归回溯（DFS）：从 (0,0) 出发随机打通墙壁，死路回溯，
/// 直到所有格子被访问过。产生长蜿蜒走廊、死胡同最少。
fn gen_backtracker(maze: &mut Maze, rng: &mut XorShift) {
    let w = maze.width;
    let h = maze.height;
    let mut visited = vec![false; w * h];
    let mut stack: Vec<(usize, usize)> = Vec::new();

    visited[0] = true;
    stack.push((0, 0));

    while let Some(&(x, y)) = stack.last() {
        let options: Vec<(usize, usize)> = neighbor4(x, y)
            .iter()
            .copied()
            .filter(|&(nx, ny)| in_bounds(nx, ny, w, h) && !visited[idx(nx, ny, w)])
            .collect();

        if options.is_empty() {
            stack.pop(); // 死路，回溯
            continue;
        }
        let (nx, ny) = options[rng.next() as usize % options.len()];
        open_between(maze, x, y, nx, ny);
        visited[idx(nx, ny, w)] = true;
        stack.push((nx, ny));
    }
}

/// 随机 Prim：维护"邻接树中格子的未接入格子"边界集，随机抽取接入。
/// 死胡同最多、分叉最密、河道最短。
fn gen_prim(maze: &mut Maze, rng: &mut XorShift) {
    let w = maze.width;
    let h = maze.height;
    let mut visited = vec![false; w * h];
    visited[0] = true;

    // 边界条目：(格子 x, y, 树中父格 px, py)
    let mut frontier: Vec<(usize, usize, usize, usize)> = Vec::new();
    for (nx, ny) in neighbor4(0, 0) {
        if in_bounds(nx, ny, w, h) {
            frontier.push((nx, ny, 0, 0));
        }
    }

    while !frontier.is_empty() {
        let i = rng.next() as usize % frontier.len();
        let (x, y, px, py) = frontier.swap_remove(i);
        if visited[idx(x, y, w)] {
            continue; // 已通过其他边界条目接入
        }
        visited[idx(x, y, w)] = true;
        open_between(maze, px, py, x, y);
        for (nx, ny) in neighbor4(x, y) {
            if in_bounds(nx, ny, w, h) && !visited[idx(nx, ny, w)] {
                frontier.push((nx, ny, x, y));
            }
        }
    }
}

/// Kruskal：所有内部墙随机顺序，并查集判环，跨集合则打通。均衡无偏。
fn gen_kruskal(maze: &mut Maze, rng: &mut XorShift) {
    let w = maze.width;
    let h = maze.height;
    let n = w * h;

    let mut parent: Vec<usize> = (0..n).collect();
    fn find(parent: &mut [usize], mut a: usize) -> usize {
        // 找根 + 路径压缩
        let root = {
            let mut r = a;
            while parent[r] != r {
                r = parent[r];
            }
            r
        };
        while parent[a] != a {
            let p = parent[a];
            parent[a] = root;
            a = p;
        }
        root
    }

    let mut edges: Vec<(usize, usize, usize)> = Vec::new();
    for y in 0..h {
        for x in 0..w {
            for dir in [E, S] {
                let (nx, ny) = match dir {
                    E => (x + 1, y),
                    S => (x, y + 1),
                    _ => unreachable!(),
                };
                if in_bounds(nx, ny, w, h) {
                    edges.push((x, y, dir));
                }
            }
        }
    }
    shuffle(&mut edges, rng);

    for (x, y, dir) in edges {
        let (nx, ny) = match dir {
            E => (x + 1, y),
            S => (x, y + 1),
            _ => unreachable!(),
        };
        let a = find(&mut parent, idx(x, y, w));
        let b = find(&mut parent, idx(nx, ny, w));
        if a != b {
            parent[a] = b;
            open_between(maze, x, y, nx, ny);
        }
    }
}

/// Wilson：loop-erased 随机游走。每个未接入格出发随机游走，
/// 撞树时擦除路径上的环并接入，无偏生成树。
fn gen_wilson(maze: &mut Maze, rng: &mut XorShift) {
    let w = maze.width;
    let h = maze.height;
    let mut in_tree = vec![false; w * h];
    in_tree[0] = true;

    for sy in 0..h {
        for sx in 0..w {
            if in_tree[idx(sx, sy, w)] {
                continue;
            }
            let mut path: Vec<(usize, usize)> = vec![(sx, sy)];
            let mut seen_pos = vec![usize::MAX; w * h];
            seen_pos[idx(sx, sy, w)] = 0;

            'walk: loop {
                let (x, y) = *path.last().unwrap();
                let (nx, ny) = neighbor4(x, y)[rng.next() as usize % 4];
                if !in_bounds(nx, ny, w, h) {
                    continue 'walk; // 越界重选方向
                }
                if in_tree[idx(nx, ny, w)] {
                    path.push((nx, ny));
                    break 'walk;
                }
                let ni = idx(nx, ny, w);
                if seen_pos[ni] != usize::MAX {
                    // 撞上本次游走经过的格子：擦除环
                    let cut = seen_pos[ni];
                    for &(cx, cy) in path.iter().skip(cut + 1) {
                        seen_pos[idx(cx, cy, w)] = usize::MAX;
                    }
                    path.truncate(cut + 1);
                } else {
                    seen_pos[ni] = path.len();
                    path.push((nx, ny));
                }
            }

            // 路径接入树
            for pair in path.windows(2) {
                let ((x1, y1), (x2, y2)) = (pair[0], pair[1]);
                open_between(maze, x1, y1, x2, y2);
            }
            for &(cx, cy) in &path {
                in_tree[idx(cx, cy, w)] = true;
            }
        }
    }
}

/// Aldous-Broder：随机游走，经过未访问格即打通并入树，直到全部访问。
/// 无偏但期望时间较长。
fn gen_aldous_broder(maze: &mut Maze, rng: &mut XorShift) {
    let w = maze.width;
    let h = maze.height;
    let mut visited = vec![false; w * h];
    visited[0] = true;
    let mut remaining = w * h - 1;
    let (mut x, mut y) = (0usize, 0usize);

    while remaining > 0 {
        let (nx, ny) = neighbor4(x, y)[rng.next() as usize % 4];
        if !in_bounds(nx, ny, w, h) {
            continue;
        }
        if !visited[idx(nx, ny, w)] {
            open_between(maze, x, y, nx, ny);
            visited[idx(nx, ny, w)] = true;
            remaining -= 1;
        }
        x = nx;
        y = ny;
    }
}

/// 生长树：活跃格列表，按 newest_ratio%（0-100）取"最新格"，否则随机格；
/// 向未访问邻居打通。newest=100 ≈ DFS，newest=0 ≈ Prim，可调支路度。
fn gen_growing_tree(maze: &mut Maze, rng: &mut XorShift, newest_ratio: u64) {
    let w = maze.width;
    let h = maze.height;
    let mut visited = vec![false; w * h];
    visited[0] = true;
    let mut active: Vec<(usize, usize)> = vec![(0, 0)];

    while !active.is_empty() {
        let (x, y) = if rng.next() % 100 < newest_ratio {
            *active.last().unwrap()
        } else {
            active[rng.next() as usize % active.len()]
        };

        let options: Vec<(usize, usize)> = neighbor4(x, y)
            .iter()
            .copied()
            .filter(|&(nx, ny)| in_bounds(nx, ny, w, h) && !visited[idx(nx, ny, w)])
            .collect();

        if options.is_empty() {
            active.retain(|&c| c != (x, y)); // 无路可走，移出活跃列表
            continue;
        }
        let (nx, ny) = options[rng.next() as usize % options.len()];
        open_between(maze, x, y, nx, ny);
        visited[idx(nx, ny, w)] = true;
        active.push((nx, ny));
    }
}

// ---------- 环状化 ----------

/// 在（完美）迷宫中随机拆掉 `count` 面内部墙，每拆一面新增一个独立环。
/// `avoid_deadends=true` 时跳过与死胡同（仅 1 条通路的格子）相邻的墙，
/// 支路 100% 保留；`false` 则为 braided 风格（拆墙吃死胡同）。
pub fn carve_loops(maze: &mut Maze, rng: &mut XorShift, count: usize, avoid_deadends: bool) {
    let w = maze.width;
    let h = maze.height;
    let mut candidates: Vec<(usize, usize, usize)> = Vec::new();
    for y in 0..h {
        for x in 0..w {
            for dir in [E, S] {
                let (nx, ny) = match dir {
                    E => (x + 1, y),
                    S => (x, y + 1),
                    _ => unreachable!(),
                };
                if !in_bounds(nx, ny, w, h) {
                    continue; // 只拆内部墙
                }
                if !maze.walls[idx(x, y, w)][dir] {
                    continue; // 已打通
                }
                if avoid_deadends && (open_degree(maze, x, y) == 1 || open_degree(maze, nx, ny) == 1) {
                    continue;
                }
                candidates.push((x, y, dir));
            }
        }
    }
    shuffle(&mut candidates, rng);
    for &(x, y, dir) in candidates.iter().take(count) {
        let (nx, ny) = match dir {
            E => (x + 1, y),
            S => (x, y + 1),
            _ => unreachable!(),
        };
        open_between(maze, x, y, nx, ny);
    }
}

// ---------- 统计指标 ----------

/// 死胡同数：恰好 1 条通路的格子
pub fn dead_end_count(maze: &Maze) -> usize {
    let w = maze.width;
    let h = maze.height;
    let mut n = 0;
    for y in 0..h {
        for x in 0..w {
            if open_degree(maze, x, y) == 1 {
                n += 1;
            }
        }
    }
    n
}

/// 独立环数（边数 - 顶点数 + 1；连通图下 = 拆墙数）
pub fn cycle_count(maze: &Maze) -> usize {
    let w = maze.width;
    let h = maze.height;
    let mut edges = 0usize;
    for y in 0..h {
        for x in 0..w {
            let i = idx(x, y, w);
            if !maze.walls[i][E] {
                edges += 1;
            }
            if !maze.walls[i][S] {
                edges += 1;
            }
        }
    }
    edges + 1 - w * h
}

/// 是否全连通（从 (0,0) BFS 可达所有格子）；测试用
#[cfg(test)]
pub(crate) fn is_connected(maze: &Maze) -> bool {
    let w = maze.width;
    let h = maze.height;
    let mut seen = vec![false; w * h];
    let mut queue = std::collections::VecDeque::new();
    seen[0] = true;
    queue.push_back((0usize, 0usize));
    while let Some((x, y)) = queue.pop_front() {
        for dir in [N, E, S, W] {
            if maze.walls[idx(x, y, w)][dir] {
                continue;
            }
            let (nx, ny) = match dir {
                N => (x, y.wrapping_sub(1)),
                E => (x + 1, y),
                S => (x, y + 1),
                W => (x.wrapping_sub(1), y),
                _ => unreachable!(),
            };
            if in_bounds(nx, ny, w, h) && !seen[idx(nx, ny, w)] {
                seen[idx(nx, ny, w)] = true;
                queue.push_back((nx, ny));
            }
        }
    }
    seen.iter().all(|&v| v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_generators_are_connected_trees() {
        for gen in Generator::ALL {
            for &size in &[5usize, 15, 31] {
                let maze = generate_seeded(size, gen, None, 42);
                assert!(is_connected(&maze), "{:?}: maze disconnected", gen);
                assert_eq!(cycle_count(&maze), 0, "{:?}: should be a tree", gen);
            }
        }
    }

    #[test]
    fn carve_loops_adds_cycles_and_preserves_dead_ends() {
        for gen in [Generator::RecursiveBacktracker, Generator::RandomizedPrim] {
            let mut maze = generate_seeded(15, gen, None, 7);
            let dead_before = dead_end_count(&maze);
            carve_loops(&mut maze, &mut XorShift::seeded(99), 15, true);
            assert_eq!(cycle_count(&maze), 15, "{:?}: each carved wall adds one cycle", gen);
            assert!(is_connected(&maze), "{:?}: became disconnected", gen);
            assert_eq!(
                dead_end_count(&maze),
                dead_before,
                "{:?}: dead ends must be preserved when avoid_deadends",
                gen
            );
        }
    }

    #[test]
    fn prim_is_branchier_than_backtracker() {
        let mut dfs = 0usize;
        let mut prim = 0usize;
        for seed in 0..20u64 {
            dfs += dead_end_count(&generate_seeded(31, Generator::RecursiveBacktracker, None, seed));
            prim += dead_end_count(&generate_seeded(31, Generator::RandomizedPrim, None, seed));
        }
        assert!(prim > dfs * 2, "Prim dead ends {} should be >2x DFS {}", prim, dfs);
    }

    #[test]
    fn game_style_generation_has_loops_and_branches() {
        // 默认玩法配置：15x15、Prim、5% 环
        let maze = generate_seeded(15, Generator::RandomizedPrim, Some(11), 3);
        assert!(is_connected(&maze));
        assert_eq!(cycle_count(&maze), 11);
        assert!(dead_end_count(&maze) > 30, "game maze should be branchy");
    }

    #[test]
    fn generator_cycle_order() {
        let mut g = Generator::ALL[0];
        for _ in 0..Generator::ALL.len() {
            g = g.next();
        }
        assert_eq!(g, Generator::ALL[0], "next() should cycle");
    }

    #[test]
    fn growing_tree_ratio_tunes_branchiness() {
        // newest 比例越低越接近 Prim（多支路），越高越接近 DFS（少支路）
        let mut high = 0usize; // 90% 最新：少支路
        let mut low = 0usize; // 10% 最新：多支路
        for seed in 0..20u64 {
            high += dead_end_count(&generate_seeded(
                31,
                Generator::GrowingTree { newest_ratio: 90 },
                None,
                seed,
            ));
            low += dead_end_count(&generate_seeded(
                31,
                Generator::GrowingTree { newest_ratio: 10 },
                None,
                seed,
            ));
        }
        assert!(
            low > high,
            "low newest ratio should be branchier (low {} vs high {})",
            low,
            high
        );
    }

    #[test]
    fn with_gt_ratio_only_affects_growing_tree() {
        let g = Generator::RandomizedPrim.with_gt_ratio(30);
        assert_eq!(g, Generator::RandomizedPrim);
        let g = Generator::GrowingTree { newest_ratio: 60 }.with_gt_ratio(30);
        assert_eq!(g, Generator::GrowingTree { newest_ratio: 30 });
        assert_eq!(
            Generator::GrowingTree { newest_ratio: 30 }.label(),
            "Growing Tree (30%)"
        );
    }
}
