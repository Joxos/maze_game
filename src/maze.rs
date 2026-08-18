//! 迷宫数据模型与求解。
//!
//! 每个格子记录四面墙（北/东/南/西），墙为 `true` 表示存在。
//! 生成算法已独立到 `maze_gen` 模块（支持多种生成器与环状化）。

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
    /// walls[i][dir]，i = y * width + x；同 crate 的生成模块可直接访问
    pub(crate) walls: Vec<[bool; 4]>,
}

impl Maze {
    fn idx(&self, x: usize, y: usize) -> usize {
        y * self.width + x
    }

    /// 查询 (x, y) 格子在 dir 方向是否有墙
    pub fn wall(&self, x: usize, y: usize, dir: usize) -> bool {
        self.walls[self.idx(x, y)][dir]
    }

    /// BFS 求解从 start 到 goal 的最短路径，返回按顺序排列的格子坐标
    /// （含起点和终点）。若不可达则返回空路径（生成保证连通，不会发生）。
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
    fn solve_finds_path() {
        // 手工迷宫：第 0 行整条走廊 + (0,0) 到 (0,1) 门洞
        let maze = test_maze(
            5,
            5,
            &[(0, 0, S), (0, 1, E), (1, 1, E), (2, 1, E), (3, 1, E)],
        );
        let path = maze.solve((0, 0), (4, 1));
        assert!(!path.is_empty());
        assert_eq!(path.first(), Some(&(0, 0)));
        assert_eq!(path.last(), Some(&(4, 1)));
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
