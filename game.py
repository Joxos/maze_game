import arcade
from maze import generate_complex_maze
import time


def grid_to_central_coordinate(row, column, cell_width):
    x = (
        column * cell_width  # column starts from 0
        + cell_width / 2
    )
    y = (
        row * cell_width  # row starts from 0
        + cell_width / 2
    )
    return [x, y]


class Game(arcade.Window):
    def __init__(self, row_count, column_count, cell_width, limited_viewport=False):
        super().__init__(row_count * cell_width, column_count * cell_width)
        self.limited_viewport = limited_viewport
        self.row_count = row_count
        self.column_count = column_count
        self.cell_width = cell_width
        self.maze = generate_complex_maze(column_count, row_count)
        self.cell_sprites = arcade.SpriteList()
        self.player_sprite = None
        self.player_position = (0, 0)
        self.is_first_move = True
        self.start_time = None
        self.move_direction = (0, 0)
        self.move_timer = 0
        self.key_stack = []
        self.key_dict = {
            arcade.key.W: (1, 0),
            arcade.key.A: (0, -1),
            arcade.key.S: (-1, 0),
            arcade.key.D: (0, 1),
        }

        self.blank_texture = arcade.make_soft_square_texture(
            cell_width, (255, 255, 255)
        )
        self.wall_texture = arcade.make_soft_square_texture(cell_width, (0, 0, 0))
        self.exit_texture = arcade.make_soft_square_texture(cell_width, (0, 255, 0))
        self.generate_maze_and_draw()
        self.generate_player_sprite()

    def generate_maze_and_draw(self):
        for row in range(self.row_count):
            for column in range(self.column_count):
                sprite = arcade.Sprite()
                sprite.center_x, sprite.center_y = grid_to_central_coordinate(
                    row, column, self.cell_width
                )
                sprite.texture = (
                    self.blank_texture
                    if self.maze[row][column] == 0
                    else self.wall_texture
                )
                if self.maze[row][column] == 2:
                    sprite.texture = self.exit_texture
                self.cell_sprites.append(sprite)

    def generate_player_sprite(self):
        for row in range(self.row_count):
            for column in range(self.column_count):
                if not self.maze[row][column]:
                    self.player_position = (row, column)
                    break
            if self.player_position != (0, 0):
                break
        self.player_sprite = arcade.SpriteCircle(
            self.cell_width // 2, arcade.color.BLUE
        )
        (
            self.player_sprite.center_x,
            self.player_sprite.center_y,
        ) = grid_to_central_coordinate(*self.player_position, self.cell_width)

    def on_draw(self):
        self.clear()
        self.player_sprite.draw()
        if self.limited_viewport:
            self.visible_sprites = arcade.SpriteList()
            directions = [(-1, 0), (1, 0), (0, -1), (0, 1)]
            for dx, dy in directions:
                # calculate the visible paths
                x, y = self.player_position
                while self.could_move(x + dx, y + dy):
                    x, y = x + dx, y + dy
                    sprite = arcade.Sprite()
                    sprite.center_x, sprite.center_y = grid_to_central_coordinate(
                        x, y, self.cell_width
                    )
                    sprite.texture = self.blank_texture
                    self.visible_sprites.append(sprite)
            self.visible_sprites.draw()
        else:
            self.cell_sprites.draw()

    def could_move(self, row, column):
        if (
            row < 0
            or row >= self.row_count
            or column < 0
            or column >= self.column_count
        ):
            return False
        if self.maze[row][column] == 1:
            return False
        if self.maze[row][column] == 2:
            return 2
        return True

    def move_player(self, dx, dy):
        result = self.could_move(
            self.player_position[0] + dx, self.player_position[1] + dy
        )
        self.move_timer = 0.1
        if result == 2:
            print(f"You win in {round(time.time() - self.start_time, 2)} seconds!")
            arcade.exit()
        elif result:
            self.player_position = (
                self.player_position[0] + dx,
                self.player_position[1] + dy,
            )
            (
                self.player_sprite.center_x,
                self.player_sprite.center_y,
            ) = grid_to_central_coordinate(*self.player_position, self.cell_width)

            self.set_viewport(
                left=self.player_position[1] * self.cell_width - self.width / 2,
                right=(self.player_position[1] + 1) * self.cell_width + self.width / 2,
                bottom=self.player_position[0] * self.cell_width - self.height / 2,
                top=(self.player_position[0] + 1) * self.cell_width + self.height / 2,
            )

    def on_key_press(self, key, key_modifiers):
        if self.is_first_move and key in [
            arcade.key.W,
            arcade.key.A,
            arcade.key.S,
            arcade.key.D,
        ]:
            self.is_first_move = False
            self.start_time = time.time()
            print(f"Game started at {self.start_time}")

        if key in self.key_dict:
            self.key_stack.append(key)
            self.move_direction = self.key_dict.get(key, (0, 0))

    def on_key_release(self, key, key_modifiers):
        if key in self.key_stack:
            self.key_stack.remove(key)
        self.move_direction = (
            self.key_dict.get(self.key_stack[-1], (0, 0)) if self.key_stack else (0, 0)
        )

    def on_update(self, delta_time: float):
        if self.move_timer <= 0:
            self.move_player(*self.move_direction)
        self.move_timer -= delta_time if self.key_stack else 0
