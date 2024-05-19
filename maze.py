import random


def find(parent, cell):
    if parent[cell] != cell:
        parent[cell] = find(parent, parent[cell])
    return parent[cell]


def union(parent, rank, cell1, cell2):
    root1 = find(parent, cell1)
    root2 = find(parent, cell2)
    if root1 != root2:
        if rank[root1] < rank[root2]:
            parent[root1] = root2
        elif rank[root1] > rank[root2]:
            parent[root2] = root1
        else:
            parent[root2] = root1
            rank[root1] += 1


def kruskal_maze_generation(width, height):
    maze = [[1 for _ in range(width)] for _ in range(height)]
    walls = []
    parent = {(x, y): (x, y) for x in range(width) for y in range(height)}
    rank = {(x, y): 0 for x in range(width) for y in range(height)}

    for y in range(height):
        for x in range(width):
            if x < width - 1:
                walls.append(((x, y), (x + 1, y)))
            if y < height - 1:
                walls.append(((x, y), (x, y + 1)))

    random.shuffle(walls)

    for wall in walls:
        cell1, cell2 = wall
        if find(parent, cell1) != find(parent, cell2):
            union(parent, rank, cell1, cell2)
            if cell1[0] == cell2[0]:  # vertical wall
                maze[cell1[1]][cell1[0]] = 0
            else:  # horizontal wall
                maze[cell1[1]][cell1[0]] = 0

    return maze


def generate_complex_maze(width, height):
    maze = [[1 for _ in range(width)] for _ in range(height)]
    directions = [(-1, 0), (1, 0), (0, -1), (0, 1)]

    def dfs(x, y):
        maze[y][x] = 0
        random.shuffle(directions)
        for dx, dy in directions:
            nx, ny = x + dx * 2, y + dy * 2
            if 0 <= nx < width and 0 <= ny < height and maze[ny][nx] == 1:
                maze[y + dy][x + dx] = 0
                dfs(nx, ny)

    start_x, start_y = (
        random.randint(0, width // 2) * 2,
        random.randint(0, height // 2) * 2,
    )
    dfs(start_x, start_y)

    min_x, max_x = 0, width - 1
    min_y, max_y = 0, height - 1

    while True:
        exit_x, exit_y = random.choice(
            [
                (x, y)
                for x in range(min_x, max_x + 1)
                for y in range(min_y, max_y + 1)
                if maze[y][x] == 0
            ]
        )
        if maze[exit_y][exit_x] == 0:
            maze[exit_y][exit_x] = 2
            break

    return maze
