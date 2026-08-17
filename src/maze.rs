//! 迷宫数据模型：生成（迭代式递归回溯 DFS）与求解（BFS）。
//!
//! 每个格子记录四面墙（北/东/南/西），墙为 `true` 表示存在。
//! 生成算法保证迷宫是"完美迷宫"：任意两个格子之间只有唯一一条路径。

/// 方向常量：北 / 东 / 南 / 西
pub const N: usize = 0;
pub const E: usize = 1;
pub const S: usize = 2;
pub const W: usize = 3;

/// 取反方向（N <-> S, E <-> W）
pub fn opposite(d: usize) -> usize {
    (d + 2) % 4
}

pub struct Maze {
    pub width: usize,
    pub height: usize,
    /// walls[i][dir]，i = y * width + x
    walls: Vec<[bool; 4]>,
}

impl Maze {
    /// 生成一个 width x height 的随机完美迷宫
    pub fn new(width: usize, height: usize) -> Self {
        let mut maze = Maze {
            width,
            height,
            walls: vec![[true; 4]; width * height],
        };
        maze.generate();
        maze
    }

    fn idx(&self, x: usize, y: usize) -> usize {
        y * self.width + x
    }

    /// 查询 (x, y) 格子在 dir 方向是否有墙
    pub fn wall(&self, x: usize, y: usize, dir: usize) -> bool {
        self.walls[self.idx(x, y)][dir]
    }

    /// 迭代式递归回溯（深度优先）：从 (0,0) 出发随机打通墙壁，
    /// 直到所有格子都被访问过。
    fn generate(&mut self) {
        let mut rng = XorShift::from_time();
        let mut visited = vec![false; self.width * self.height];
        let mut stack: Vec<(usize, usize)> = Vec::new();

        visited[0] = true;
        stack.push((0, 0));

        while let Some(&(x, y)) = stack.last() {
            // 收集未访问的相邻格子
            let mut options: Vec<(usize, usize, usize)> = Vec::new();
            if x > 0 && !visited[self.idx(x - 1, y)] {
                options.push((x - 1, y, W));
            }
            if x + 1 < self.width && !visited[self.idx(x + 1, y)] {
                options.push((x + 1, y, E));
            }
            if y > 0 && !visited[self.idx(x, y - 1)] {
                options.push((x, y - 1, N));
            }
            if y + 1 < self.height && !visited[self.idx(x, y + 1)] {
                options.push((x, y + 1, S));
            }

            if options.is_empty() {
                // 死胡同，回溯
                stack.pop();
                continue;
            }

            // 随机选一个邻居并打通中间的墙
            let (nx, ny, dir) = options[rng.next() as usize % options.len()];
            let i = self.idx(x, y);
            let j = self.idx(nx, ny);
            self.walls[i][dir] = false;
            self.walls[j][opposite(dir)] = false;
            visited[self.idx(nx, ny)] = true;
            stack.push((nx, ny));
        }
    }

    /// BFS 求解从 start 到 goal 的最短路径，返回按顺序排列的格子坐标
    /// （含起点和终点）。若不可达则返回空路径（完美迷宫下不会发生）。
    pub fn solve(&self, start: (usize, usize), goal: (usize, usize)) -> Vec<(usize, usize)> {
        let mut prev: Vec<Option<(usize, usize)>> = vec![None; self.width * self.height];
        let mut queue = std::collections::VecDeque::new();

        prev[self.idx(start.0, start.1)] = Some(start);
        queue.push_back(start);

        while let Some((x, y)) = queue.pop_front() {
            if (x, y) == goal {
                break;
            }
            for (dir, (dx, dy)) in [(N, (0i32, -1i32)), (E, (1, 0)), (S, (0, 1)), (W, (-1, 0))] {
                if self.wall(x, y, dir) {
                    continue; // 外墙始终存在，越界访问已被墙挡住
                }
                let (nx, ny) = ((x as i32 + dx) as usize, (y as i32 + dy) as usize);
                if prev[self.idx(nx, ny)].is_some() {
                    continue;
                }
                prev[self.idx(nx, ny)] = Some((x, y));
                queue.push_back((nx, ny));
            }
        }

        // 回溯重建路径
        let mut path = Vec::new();
        let mut cur = goal;
        loop {
            path.push(cur);
            if cur == start {
                break;
            }
            match prev[self.idx(cur.0, cur.1)] {
                Some(p) => cur = p,
                None => break,
            }
        }
        path.reverse();
        path
    }
}

/// 简单的 xorshift64 伪随机数生成器（避免额外依赖）
struct XorShift(u64);

impl XorShift {
    /// 用当前系统时间（纳秒）做种子
    fn from_time() -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        XorShift(seed | 1)
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

/// 测试用：手工构造迷宫，`open` 为要打通的墙段列表 (x, y, dir)，
/// 同时打通相邻格子的反向墙。仅测试编译时可用。
#[cfg(test)]
pub(crate) fn test_maze(width: usize, height: usize, open: &[(usize, usize, usize)]) -> Maze {
    let mut maze = Maze {
        width,
        height,
        walls: vec![[true; 4]; width * height],
    };
    for &(x, y, dir) in open {
        let (dx, dy) = match dir {
            N => (0i32, -1i32),
            E => (1, 0),
            S => (0, 1),
            W => (-1, 0),
            _ => unreachable!(),
        };
        maze.walls[y * width + x][dir] = false;
        let (nx, ny) = ((x as i32 + dx) as usize, (y as i32 + dy) as usize);
        if nx < width && ny < height {
            maze.walls[ny * width + nx][opposite(dir)] = false;
        }
    }
    maze
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maze_is_connected() {
        // 完美迷宫：从起点 BFS 应能到达所有格子
        for w in [5usize, 11, 15, 31] {
            let maze = Maze::new(w, w);
            let mut seen = vec![false; w * w];
            let mut queue = std::collections::VecDeque::new();
            seen[0] = true;
            queue.push_back((0usize, 0usize));
            while let Some((x, y)) = queue.pop_front() {
                for (dir, (dx, dy)) in [(N, (0i32, -1i32)), (E, (1, 0)), (S, (0, 1)), (W, (-1, 0))] {
                    if maze.wall(x, y, dir) {
                        continue;
                    }
                    let (nx, ny) = ((x as i32 + dx) as usize, (y as i32 + dy) as usize);
                    if !seen[maze.idx(nx, ny)] {
                        seen[maze.idx(nx, ny)] = true;
                        queue.push_back((nx, ny));
                    }
                }
            }
            assert!(seen.iter().all(|&v| v), "maze {}x{} is not fully connected", w, w);
        }
    }

    #[test]
    fn solve_finds_path() {
        let maze = Maze::new(15, 15);
        let path = maze.solve((0, 0), (14, 14));
        assert!(!path.is_empty());
        assert_eq!(path.first(), Some(&(0, 0)));
        assert_eq!(path.last(), Some(&(14, 14)));
        // 路径上的相邻格子之间不能有墙
        for pair in path.windows(2) {
            let (x1, y1) = pair[0];
            let (x2, y2) = pair[1];
            let dx = x2 as i32 - x1 as i32;
            let dy = y2 as i32 - y1 as i32;
            let dir = match (dx, dy) {
                (1, 0) => E,
                (-1, 0) => W,
                (0, 1) => S,
                _ => N,
            };
            assert!(!maze.wall(x1, y1, dir));
        }
    }
}
