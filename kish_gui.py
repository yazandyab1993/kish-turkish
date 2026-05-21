import tkinter as tk
from tkinter import messagebox, ttk
import random
import re
import time
import kish


FILES = "abcdefgh"


class SearchTimeout(Exception):
    pass


class KishGUI:
    def __init__(self, root):
        self.root = root
        self.root.title("Turkish Dama - Kish Engine GUI")
        self.root.geometry("1020x830")
        self.root.resizable(False, False)

        self.game = kish.Game()
        self.selected_square = None

        self.ai_busy = False
        self.ai_after_id = None

        self.human_color = "White"
        self.ai_color = "Black"
        self.board_flipped = False

        self.ai_max_depth = 8
        self.ai_time_limit_ms = 1200
        self.search_deadline = 0.0
        self.tt = {}
        self.current_search_depth = 0
        self.last_analysis_rows = []

        self.cell_size = 72
        self.board_size = self.cell_size * 8

        self.main_frame = tk.Frame(root, bg="#2b2b2b")
        self.main_frame.pack(fill="both", expand=True)

        self.left_frame = tk.Frame(self.main_frame, bg="#2b2b2b")
        self.left_frame.pack(side="left", fill="y")

        self.analysis_frame = tk.Frame(self.main_frame, bg="#1f1f1f", width=300)
        self.analysis_frame.pack(side="right", fill="y")
        self.analysis_frame.pack_propagate(False)

        self.top_frame = tk.Frame(self.left_frame, bg="#1f1f1f")
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

        controls = tk.Frame(self.top_frame, bg="#1f1f1f")
        controls.pack(side="right", padx=10)

        self.depth_var = tk.IntVar(value=self.ai_max_depth)
        tk.Label(controls, text="Depth", bg="#1f1f1f", fg="white").pack(side="left", padx=(0, 4))
        self.depth_spin = tk.Spinbox(
            controls,
            from_=1,
            to=20,
            width=4,
            textvariable=self.depth_var,
            command=self.on_settings_changed
        )
        self.depth_spin.pack(side="left", padx=(0, 10))

        self.time_var = tk.IntVar(value=self.ai_time_limit_ms)
        tk.Label(controls, text="Time(ms)", bg="#1f1f1f", fg="white").pack(side="left", padx=(0, 4))
        self.time_spin = tk.Spinbox(
            controls,
            from_=200,
            to=10000,
            increment=100,
            width=6,
            textvariable=self.time_var,
            command=self.on_settings_changed
        )
        self.time_spin.pack(side="left", padx=(0, 10))

        self.color_var = tk.StringVar(value="White")
        tk.Label(controls, text="Play as", bg="#1f1f1f", fg="white").pack(side="left", padx=(0, 4))
        self.color_menu = tk.OptionMenu(controls, self.color_var, "White", "Black", command=self.on_color_changed)
        self.color_menu.pack(side="left", padx=(0, 10))

        self.flip_btn = tk.Button(
            controls,
            text="Flip Board",
            font=("Segoe UI", 10, "bold"),
            command=self.toggle_board_flip
        )
        self.flip_btn.pack(side="left", padx=(0, 10))

        self.analyze_btn = tk.Button(
            controls,
            text="Analyze",
            font=("Segoe UI", 10, "bold"),
            command=self.analyze_current_position
        )
        self.analyze_btn.pack(side="left", padx=(0, 10))

        self.new_btn = tk.Button(
            controls,
            text="New Game",
            font=("Segoe UI", 10, "bold"),
            command=self.new_game
        )
        self.new_btn.pack(side="left", padx=(0, 6))

        self.undo_btn = tk.Button(
            controls,
            text="Undo",
            font=("Segoe UI", 10, "bold"),
            command=self.undo
        )
        self.undo_btn.pack(side="left")

        self.canvas = tk.Canvas(
            self.left_frame,
            width=self.board_size,
            height=self.board_size,
            bg="#222",
            highlightthickness=0
        )
        self.canvas.pack(pady=20)
        self.canvas.bind("<Button-1>", self.on_click)

        self.info_label = tk.Label(
            self.left_frame,
            text="",
            font=("Segoe UI", 11),
            fg="#ddd",
            bg="#2b2b2b",
            pady=8
        )
        self.info_label.pack(fill="x")

        self.setup_analysis_panel()

        self.root.configure(bg="#2b2b2b")

        self.update_info_label()
        self.draw()
        self.maybe_schedule_ai_move()

    def setup_analysis_panel(self):
        title = tk.Label(
            self.analysis_frame,
            text="Move Analysis",
            font=("Segoe UI", 13, "bold"),
            bg="#1f1f1f",
            fg="white",
            pady=10
        )
        title.pack(fill="x")

        columns = ("rank", "move", "score", "depth", "capture", "promotion")
        self.analysis_tree = ttk.Treeview(self.analysis_frame, columns=columns, show="headings", height=26)
        self.analysis_tree.heading("rank", text="#")
        self.analysis_tree.heading("move", text="Move")
        self.analysis_tree.heading("score", text="Score")
        self.analysis_tree.heading("depth", text="Depth")
        self.analysis_tree.heading("capture", text="Cap")
        self.analysis_tree.heading("promotion", text="Prom")

        self.analysis_tree.column("rank", width=35, anchor="center")
        self.analysis_tree.column("move", width=95, anchor="center")
        self.analysis_tree.column("score", width=65, anchor="e")
        self.analysis_tree.column("depth", width=50, anchor="center")
        self.analysis_tree.column("capture", width=45, anchor="center")
        self.analysis_tree.column("promotion", width=55, anchor="center")

        self.analysis_tree.pack(fill="both", expand=True, padx=8, pady=(0, 8))

    def update_analysis_panel(self, rows):
        self.last_analysis_rows = rows
        for item in self.analysis_tree.get_children():
            self.analysis_tree.delete(item)

        for i, row in enumerate(rows, start=1):
            self.analysis_tree.insert(
                "",
                "end",
                values=(
                    i,
                    row["move"],
                    f"{row['score']:.2f}",
                    row["depth"],
                    "Y" if row["capture"] else "-",
                    "Y" if row["promotion"] else "-",
                )
            )

    def update_info_label(self):
        board_view = "Flipped" if self.board_flipped else "Normal"
        self.info_label.config(
            text=f"You: {self.human_color} | AI: {self.ai_color} | Board: {board_view}. Click a piece, then click destination."
        )

    def on_settings_changed(self):
        try:
            self.ai_max_depth = max(1, min(20, int(self.depth_var.get())))
        except Exception:
            self.ai_max_depth = 8
            self.depth_var.set(self.ai_max_depth)

        try:
            self.ai_time_limit_ms = max(200, min(10000, int(self.time_var.get())))
        except Exception:
            self.ai_time_limit_ms = 1200
            self.time_var.set(self.ai_time_limit_ms)

        self.draw()

    def on_color_changed(self, value):
        self.human_color = str(value)
        self.ai_color = "Black" if self.human_color == "White" else "White"
        self.board_flipped = self.human_color == "Black"
        self.new_game()

    def toggle_board_flip(self):
        self.board_flipped = not self.board_flipped
        self.update_info_label()
        self.draw()

    def new_game(self):
        self.on_settings_changed()
        self.game = kish.Game()
        self.selected_square = None
        self.tt.clear()
        self.update_info_label()
        self.update_analysis_panel([])
        self.draw()
        self.maybe_schedule_ai_move()

    def undo(self):
        if self.ai_after_id is not None:
            self.root.after_cancel(self.ai_after_id)
            self.ai_after_id = None
        self.ai_busy = False

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
        text = str(self.game.board())
        board = {}

        for line in text.splitlines():
            line = line.strip()
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
        notation = str(action)
        coords = re.findall(r"[a-h][1-8]", notation.lower())
        if len(coords) < 2:
            return None, None
        return coords[0], coords[-1]

    def get_actions_from_square(self, square):
        return [a for a in self.game.actions() if self.action_coords(a)[0] == square]

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

        if self.board_flipped:
            board_col = 7 - col
            board_rank = row + 1
        else:
            board_col = col
            board_rank = 8 - row

        return f"{FILES[board_col]}{board_rank}"

    def is_human_turn(self):
        turn_text = str(self.game.turn()) if callable(self.game.turn) else str(self.game.turn)
        return self.human_color in turn_text

    def is_ai_turn(self):
        turn_text = str(self.game.turn()) if callable(self.game.turn) else str(self.game.turn)
        return self.ai_color in turn_text

    def draw(self):
        self.canvas.delete("all")

        board = self.get_board_matrix()

        current_turn = str(self.game.turn()) if callable(self.game.turn) else str(self.game.turn)
        status = str(self.game.status()) if callable(self.game.status) else str(self.game.status)

        self.status_label.config(
            text=f"Turn: {current_turn} | Status: {status} | Depth: {self.ai_max_depth} | Time: {self.ai_time_limit_ms}ms"
        )

        legal_destinations = set()
        if self.selected_square:
            for action in self.get_actions_from_square(self.selected_square):
                _, dst = self.action_coords(action)
                if dst:
                    legal_destinations.add(dst)

        for display_row in range(8):
            for display_col in range(8):
                x1 = display_col * self.cell_size
                y1 = display_row * self.cell_size
                x2 = x1 + self.cell_size
                y2 = y1 + self.cell_size

                if self.board_flipped:
                    file_index = 7 - display_col
                    rank = display_row + 1
                else:
                    file_index = display_col
                    rank = 8 - display_row

                square = f"{FILES[file_index]}{rank}"
                color = "#d8b47b" if (display_row + display_col) % 2 == 0 else "#8b5a2b"

                if square == self.selected_square:
                    color = "#e0d060"
                elif square in legal_destinations:
                    color = "#7fc97f"

                self.canvas.create_rectangle(x1, y1, x2, y2, fill=color, outline="#2a1c10", width=2)
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

        self.canvas.create_oval(cx - r, cy - r, cx + r, cy + r, fill=fill, outline=outline, width=3)

        if is_king:
            self.canvas.create_text(cx, cy, text="K", fill=text_color, font=("Segoe UI", 18, "bold"))

    def on_click(self, event):
        if self.ai_busy or not self.is_human_turn():
            return

        square = self.coord_to_square(event.x, event.y)
        if not square:
            return

        board = self.get_board_matrix()
        piece = board.get(square, ".")
        player_piece_prefix = "w" if self.human_color == "White" else "b"

        if self.selected_square is None:
            if piece.lower().startswith(player_piece_prefix):
                actions = self.get_actions_from_square(square)
                if actions:
                    self.selected_square = square
                    self.draw()
            return

        action = self.find_action(self.selected_square, square)

        if action:
            self.make_player_move(action)
        else:
            if piece.lower().startswith(player_piece_prefix):
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
        self.maybe_schedule_ai_move()

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
        white_men, white_kings, black_men, black_kings = self.get_piece_counts_from_board()

        white_score = (white_men * 100) + (white_kings * 280)
        black_score = (black_men * 100) + (black_kings * 280)
        score = white_score - black_score

        try:
            mobility = len(self.game.actions())
            turn_text = str(self.game.turn()) if callable(self.game.turn) else str(self.game.turn)
            if "White" in turn_text:
                score += mobility * 2
            elif "Black" in turn_text:
                score -= mobility * 2
        except Exception:
            pass

        return score

    def move_order_score(self, action):
        score = 0
        is_capture = bool(self.get_action_value(action, "is_capture", False))
        capture_count = int(self.get_action_value(action, "capture_count", 0) or 0)
        is_promotion = bool(self.get_action_value(action, "is_promotion", False))

        if is_capture:
            score += 2000 + capture_count * 300
        if is_promotion:
            score += 700

        return score

    def game_state_key(self):
        turn_text = str(self.game.turn()) if callable(self.game.turn) else str(self.game.turn)
        return f"{turn_text}|{str(self.game.board())}"

    def search_time_exceeded(self):
        return time.perf_counter() >= self.search_deadline

    def negamax(self, depth, alpha, beta, color):
        if self.search_time_exceeded():
            raise SearchTimeout()

        state_key = self.game_state_key()
        tt_key = (state_key, depth, color)
        if tt_key in self.tt:
            return self.tt[tt_key]

        actions = self.game.actions()
        if depth == 0 or not actions:
            value = color * self.evaluate_position()
            self.tt[tt_key] = value
            return value

        ordered = sorted(actions, key=self.move_order_score, reverse=True)
        best_score = -10**9

        for action in ordered:
            self.game.make_move(action)
            try:
                score = -self.negamax(depth - 1, -beta, -alpha, -color)
            finally:
                self.game.undo_move()

            if score > best_score:
                best_score = score

            if score > alpha:
                alpha = score
            if alpha >= beta:
                break

        self.tt[tt_key] = best_score
        return best_score

    def get_ai_color_sign(self):
        return 1 if self.ai_color == "White" else -1

    def analyze_current_position(self):
        if self.ai_busy:
            return
        self.on_settings_changed()
        self.tt.clear()
        _, analysis_rows = self.find_best_move(self.ai_max_depth, self.ai_time_limit_ms, collect_analysis=True)
        self.update_analysis_panel(analysis_rows)

    def find_best_move(self, max_depth, time_limit_ms, collect_analysis=False):
        actions = self.game.actions()
        if not actions:
            return None, []

        self.search_deadline = time.perf_counter() + (time_limit_ms / 1000.0)
        ordered_root = sorted(actions, key=self.move_order_score, reverse=True)

        fallback = random.choice(actions)
        best_overall = ordered_root[0] if ordered_root else fallback
        ai_sign = self.get_ai_color_sign()

        best_analysis = []

        for depth in range(1, max_depth + 1):
            if self.search_time_exceeded():
                break

            self.current_search_depth = depth
            ranked = []

            try:
                for action in ordered_root:
                    if self.search_time_exceeded():
                        raise SearchTimeout()

                    self.game.make_move(action)
                    try:
                        score = -self.negamax(depth - 1, -10**9, 10**9, -ai_sign)
                    finally:
                        self.game.undo_move()

                    score += self.move_order_score(action) * 0.001
                    ranked.append((score, action))

                ranked.sort(key=lambda item: item[0], reverse=True)
                if ranked:
                    best_overall = ranked[0][1]
                    ordered_root = [item[1] for item in ranked]

                if collect_analysis:
                    best_analysis = [
                        {
                            "move": str(act),
                            "score": val,
                            "depth": depth,
                            "capture": bool(self.get_action_value(act, "is_capture", False)),
                            "promotion": bool(self.get_action_value(act, "is_promotion", False)),
                        }
                        for val, act in ranked[:12]
                    ]

            except SearchTimeout:
                break

        return best_overall, best_analysis

    def maybe_schedule_ai_move(self):
        if self.is_ai_turn() and not self.ai_busy and self.ai_after_id is None:
            self.ai_busy = True
            self.ai_after_id = self.root.after(250, self.ai_move)

    def ai_move(self):
        self.ai_after_id = None

        if not self.is_ai_turn():
            self.ai_busy = False
            self.draw()
            return

        actions = self.game.actions()
        if not actions:
            self.ai_busy = False
            self.draw()
            messagebox.showinfo("Game Over", "No legal moves.")
            return

        self.on_settings_changed()
        self.tt.clear()
        action, analysis_rows = self.find_best_move(max_depth=self.ai_max_depth, time_limit_ms=self.ai_time_limit_ms, collect_analysis=True)
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
        self.update_analysis_panel(analysis_rows)
        self.draw()


if __name__ == "__main__":
    root = tk.Tk()
    app = KishGUI(root)
    root.mainloop()
