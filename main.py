from game import Game
import arcade


if __name__ == "__main__":
    game = Game(30, 30, 20, limited_viewport=False)
    arcade.run()
