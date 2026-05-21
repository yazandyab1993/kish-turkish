import tkinter as tk
from tkinter import messagebox
import random
import re
import kish


FILES = "abcdefgh"


class KishGUI:
    def __init__(self, root):
        self.root = root
        self.root.title("Turkish Dama - Kish Engine GUI")
        self.root.geometry("720x760")
        self.root.resizable(False, False)

        self.game = kish.Game()
        self.selected_square = None
        self.legal_actions = []

        self.ai_busy = False
        self.ai_after_id = None

        self.cell_size = 72
        self.board_size = self.cell_size * 8

        self.top_frame = tk.Frame(root, bg="#1f1f1f")
        self.top_frame.pack(fill="x")

        self.status_label = tk.Label(
            self.top_frame,
            text="",
            font=("Segoe UI", 14, "bold"),
            bg="#1f1f1f",
            fg="white",
            pady=10
        )
        self.status_label.pack(side="left", padx=15)

        self.new_btn = tk.Button(
            self.top_frame,
            text="New Game",
            font=("Segoe UI", 11, "bold"),
            command=self.new_game
        )
        self.new_btn.pack(side="right", padx=10)

        self.undo_btn = tk.Button(
            self.top_frame,
            text="Undo",
            font=("Segoe UI", 11, "bold"),
            command=self.undo
        )
        self.undo_btn.pack(side="right", padx=10)

        self.canvas = tk.Canvas(
            root,
            width=self.board_size,
            height=self.board_size,
            bg="#222",
            highlightthickness=0
        )
        self.canvas.pack(pady=20)
        self.canvas.bind("<Button-1>", self.on_click)

        self.info_label = tk.Label(
            root,
            text="You play White. Click a piece, then click destination.",
            font=("Segoe UI", 11),
            fg="#ddd",
            bg="#2b2b2b",
            pady=8
        )
        self.info_label.pack(fill="x")

        self.root.configure(bg="#2b2b2b")

        self.draw()

    def new_game(self):
        self.game = kish.Game()
        self.selected_square = None
        self.draw()

    def undo(self):
        try:
            self.game.undo_move()
            self.game.undo_move()
        except Exception:
            try:
                self.game.undo_move()
            except Exception:
                pass

        self.selected_square = None
        self.draw()

    def get_board_matrix(self):
        """
        Reads board from kish textual output.
        Returns dict like:
        {
            "a1": ".",
            "a2": "w",
            ...
        }
        """
        text = str(self.game.board())
        board = {}

        for line in text.splitlines():
            line = line.strip()

            # Example:
            # 7  b b b b b b b b  7
            match = re.match(r"^([1-8])\s+(.+?)\s+\1$", line)
            if not match:
                continue

            rank = int(match.group(1))
            pieces = match.group(2).split()

            if len(pieces) != 8:
                continue

            for col, piece in enumerate(pieces):
                square = f"{FILES[col]}{rank}"
                board[square] = piece

        return board

    def action_coords(self, action):
        """
        Converts action string like:
        a3-a4
        a3xa5
        a3xc3xe3
        into source and final destination.
        """
        notation = str(action)
        coords = re.findall(r"[a-h][1-8]", notation.lower())

        if len(coords) < 2:
            return None, None

        return coords[0], coords[-1]

    def get_actions_from_square(self, square):
        result = []
        for action in self.game.actions():
            src, dst = self.action_coords(action)
            if src == square:
                result.append(action)
        return result

    def find_action(self, source, destination):
        for action in self.game.actions():
            src, dst = self.action_coords(action)
            if src == source and dst == destination:
                return action
        return None

    def coord_to_square(self, x, y):
        col = x // self.cell_size
        row = y // self.cell_size

        if not (0 <= col < 8 and 0 <= row < 8):
            return None

        file = FILES[col]
        rank = 8 - row

        return f"{file}{rank}"

    def square_to_xy(self, square):
        file = square[0]
        rank = int(square[1])

        col = FILES.index(file)
        row = 8 - rank

        x = col * self.cell_size
        y = row * self.cell_size

        return x, y

    def draw(self):
        self.canvas.delete("all")

        board = self.get_board_matrix()

        current_turn = str(self.game.turn()) if callable(self.game.turn) else str(self.game.turn)
        status = str(self.game.status()) if callable(self.game.status) else str(self.game.status)

        self.status_label.config(
            text=f"Turn: {current_turn}   |   Status: {status}"
        )

        legal_destinations = set()
        if self.selected_square:
            for action in self.get_actions_from_square(self.selected_square):
                _, dst = self.action_coords(action)
                if dst:
                    legal_destinations.add(dst)

        for row in range(8):
            for col in range(8):
                x1 = col * self.cell_size
                y1 = row * self.cell_size
                x2 = x1 + self.cell_size
                y2 = y1 + self.cell_size

                square = f"{FILES[col]}{8 - row}"

                color = "#d8b47b" if (row + col) % 2 == 0 else "#8b5a2b"

                if square == self.selected_square:
                    color = "#e0d060"
                elif square in legal_destinations:
                    color = "#7fc97f"

                self.canvas.create_rectangle(
                    x1, y1, x2, y2,
                    fill=color,
                    outline="#2a1c10",
                    width=2
                )

                # Square label
                self.canvas.create_text(
                    x1 + 8,
                    y1 + 8,
                    text=square.upper(),
                    anchor="nw",
                    fill="#2a1c10",
                    font=("Segoe UI", 8, "bold")
                )

                piece = board.get(square, ".")

                if piece != ".":
                    self.draw_piece(x1, y1, piece)

    def draw_piece(self, x, y, piece):
        cx = x + self.cell_size / 2
        cy = y + self.cell_size / 2
        r = self.cell_size * 0.34

        is_white = "w" in piece.lower()
        is_black = "b" in piece.lower()
        is_king = "k" in piece.lower() or piece.isupper()

        if is_white:
            fill = "#f5f5f5"
            outline = "#444"
            text_color = "#222"
        elif is_black:
            fill = "#171717"
            outline = "#ddd"
            text_color = "#fff"
        else:
            fill = "#999"
            outline = "#333"
            text_color = "#fff"

        self.canvas.create_oval(
            cx - r, cy - r, cx + r, cy + r,
            fill=fill,
            outline=outline,
            width=3
        )

        if is_king:
            self.canvas.create_text(
                cx,
                cy,
                text="K",
                fill=text_color,
                font=("Segoe UI", 18, "bold")
            )

    def on_click(self, event):
        if self.ai_busy:
            return

        turn_text = str(self.game.turn()) if callable(self.game.turn) else str(self.game.turn)
        if "White" not in turn_text:
            return
        square = self.coord_to_square(event.x, event.y)
        if not square:
            return

        board = self.get_board_matrix()
        piece = board.get(square, ".")

        # Player is White only in this first version
        turn_text = str(self.game.turn()) if callable(self.game.turn) else str(self.game.turn)
        if "White" not in turn_text:
            return

        if self.selected_square is None:
            if piece.lower().startswith("w"):
                actions = self.get_actions_from_square(square)
                if actions:
                    self.selected_square = square
                    self.draw()
            return

        action = self.find_action(self.selected_square, square)

        if action:
            self.make_player_move(action)
        else:
            # If clicked another white piece, select it
            if piece.lower().startswith("w"):
                actions = self.get_actions_from_square(square)
                if actions:
                    self.selected_square = square
                    self.draw()
                    return

            self.selected_square = None
            self.draw()

    def make_player_move(self, action):
        if self.ai_busy:
            return

        try:
            self.game.make_move(action)
        except Exception as e:
            messagebox.showerror("Invalid Move", str(e))
            return

        self.selected_square = None
        self.draw()

        turn_text = str(self.game.turn()) if callable(self.game.turn) else str(self.game.turn)

        # لا تستدعِ الكمبيوتر إلا إذا صار الدور للأسود فعلاً
        if "Black" in turn_text and self.ai_after_id is None:
            self.ai_busy = True
            self.ai_after_id = self.root.after(350, self.ai_move)
    def get_action_value(self, action, name, default=None):
        value = getattr(action, name, default)

        if callable(value):
            try:
                return value()
            except Exception:
                return default

        return value


    def get_piece_counts_from_board(self):
        board = self.get_board_matrix()

        white_men = 0
        black_men = 0
        white_kings = 0
        black_kings = 0

        for piece in board.values():
            p = str(piece)

            if p == ".":
                continue

            lower = p.lower()

            if "w" in lower:
                if "k" in lower or p.isupper():
                    white_kings += 1
                else:
                    white_men += 1

            elif "b" in lower:
                if "k" in lower or p.isupper():
                    black_kings += 1
                else:
                    black_men += 1

        return white_men, white_kings, black_men, black_kings


    def evaluate_position(self):
        """
        Positive score = good for Black AI
        Negative score = good for White player
        """
        white_men, white_kings, black_men, black_kings = self.get_piece_counts_from_board()

        score = 0

        # Material
        score += black_men * 100
        score += black_kings * 260
        score -= white_men * 100
        score -= white_kings * 260

        # Mobility
        try:
            turn_text = str(self.game.turn()) if callable(self.game.turn) else str(self.game.turn)
            mobility = len(self.game.actions())

            if "Black" in turn_text:
                score += mobility * 3
            elif "White" in turn_text:
                score -= mobility * 3
        except Exception:
            pass

        return score


    def move_order_score(self, action):
        score = 0

        is_capture = bool(self.get_action_value(action, "is_capture", False))
        capture_count = int(self.get_action_value(action, "capture_count", 0) or 0)
        is_promotion = bool(self.get_action_value(action, "is_promotion", False))

        if is_capture:
            score += 1000 + capture_count * 300

        if is_promotion:
            score += 500

        return score


    def negamax(self, depth, alpha, beta, color):
        """
        color:
        +1 when current side is Black AI perspective
        -1 when current side is White player perspective
        """
        actions = self.game.actions()

        if depth == 0 or not actions:
            return color * self.evaluate_position()

        # Better move ordering makes alpha-beta much faster
        actions = sorted(actions, key=self.move_order_score, reverse=True)

        best_score = -10**9

        for action in actions:
            try:
                self.game.make_move(action)
            except Exception:
                continue

            score = -self.negamax(depth - 1, -beta, -alpha, -color)

            try:
                self.game.undo_move()
            except Exception:
                pass

            if score > best_score:
                best_score = score

            alpha = max(alpha, score)

            if alpha >= beta:
                break

        return best_score


    def find_best_move(self, depth=5):
        actions = self.game.actions()

        if not actions:
            return None

        # Strong ordering before search
        actions = sorted(actions, key=self.move_order_score, reverse=True)

        best_action = None
        best_score = -10**9

        for action in actions:
            try:
                self.game.make_move(action)
            except Exception:
                continue

            score = -self.negamax(depth - 1, -10**9, 10**9, -1)

            try:
                self.game.undo_move()
            except Exception:
                pass

            # Add tiny preference for stronger tactical moves if equal
            score += self.move_order_score(action) * 0.001

            if score > best_score:
                best_score = score
                best_action = action

        print(f"AI selected: {best_action} | score: {best_score:.2f} | depth: {depth}")

        return best_action    
    def ai_move(self):
        self.ai_after_id = None

        turn_text = str(self.game.turn()) if callable(self.game.turn) else str(self.game.turn)

        # حماية مهمة: إذا لم يكن الدور للأسود، لا يتحرك الكمبيوتر
        if "Black" not in turn_text:
            self.ai_busy = False
            self.draw()
            return

        actions = self.game.actions()

        if not actions:
            self.ai_busy = False
            self.draw()
            messagebox.showinfo("Game Over", "No legal moves.")
            return

        depth = 5

        action = self.find_best_move(depth=depth)

        if action is None:
            action = random.choice(actions)

        try:
            self.game.make_move(action)
        except Exception as e:
            self.ai_busy = False
            messagebox.showerror("AI Move Error", str(e))
            return

        self.ai_busy = False
        self.selected_square = None
        self.draw()

        # Simple AI:
        # Prefer captures. If no capture, random move.
        def get_action_value(action, name, default=None):
            value = getattr(action, name, default)

            if callable(value):
                try:
                    return value()
                except Exception:
                    return default

            return value


        capture_actions = [
            a for a in actions
            if bool(get_action_value(a, "is_capture", False))
        ]

        if capture_actions:
            best_capture_count = max(
                int(get_action_value(a, "capture_count", 0) or 0)
                for a in capture_actions
            )

            best_actions = [
                a for a in capture_actions
                if int(get_action_value(a, "capture_count", 0) or 0) == best_capture_count
            ]

            action = random.choice(best_actions)
        else:
            action = random.choice(actions)

        try:
            self.game.make_move(action)
        except Exception as e:
            messagebox.showerror("AI Move Error", str(e))

        self.draw()


if __name__ == "__main__":
    root = tk.Tk()
    app = KishGUI(root)
    root.mainloop()