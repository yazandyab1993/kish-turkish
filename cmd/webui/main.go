package main

import (
	"TurkishDraughts/Board"
	"encoding/json"
	"log"
	"net/http"
	"sync"
)

type gameState struct {
	mu    sync.Mutex
	board board.BoardState
}

type stateResponse struct {
	Board      []string      `json:"board"`
	Turn       string        `json:"turn"`
	GameOver   bool          `json:"gameOver"`
	Winner     string        `json:"winner,omitempty"`
	Draw       bool          `json:"draw"`
	ValidMoves map[int][]int `json:"validMoves"`
	IsTakeMap  bool          `json:"isTakeMap"`
	Depth      int           `json:"depth"`
}

type moveRequest struct {
	From int `json:"from"`
	To   int `json:"to"`
}
type depthRequest struct {
	Depth int `json:"depth"`
}

func main() {
	g := &gameState{board: board.CreateStartingBoard()}
	board.MaxDepth = 7

	http.Handle("/", http.FileServer(http.Dir("webui")))
	http.HandleFunc("/api/state", g.handleState)
	http.HandleFunc("/api/move", g.handleMove)
	http.HandleFunc("/api/ai", g.handleAI)
	http.HandleFunc("/api/depth", g.handleDepth)
	http.HandleFunc("/api/reset", g.handleReset)

	log.Println("Web UI on http://localhost:8080")
	log.Fatal(http.ListenAndServe(":8080", nil))
}

func (g *gameState) handleState(w http.ResponseWriter, _ *http.Request) {
	g.mu.Lock()
	defer g.mu.Unlock()
	writeJSON(w, snapshot(g.board))
}
func (g *gameState) handleReset(w http.ResponseWriter, _ *http.Request) {
	g.mu.Lock()
	g.board = board.CreateStartingBoard()
	s := snapshot(g.board)
	g.mu.Unlock()
	writeJSON(w, s)
}
func (g *gameState) handleDepth(w http.ResponseWriter, r *http.Request) {
	var req depthRequest
	_ = json.NewDecoder(r.Body).Decode(&req)
	if req.Depth < 1 {
		req.Depth = 1
	}
	if req.Depth > 12 {
		req.Depth = 12
	}
	board.MaxDepth = int32(req.Depth)
	g.handleState(w, r)
}

func (g *gameState) handleMove(w http.ResponseWriter, r *http.Request) {
	var req moveRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, err.Error(), 400)
		return
	}
	g.mu.Lock()
	defer g.mu.Unlock()
	moveMap, _ := legalMoveMap(&g.board)
	if !contains(moveMap[req.From], req.To) {
		http.Error(w, "illegal move", 400)
		return
	}
	swap, dir := tryMove(&g.board, req.From, req.To)
	nextTakes := validUiTakes(&g.board, req.To, dir)
	if swap || len(nextTakes) == 0 {
		g.board.SwapTeam()
	}
	writeJSON(w, snapshot(g.board))
}

func (g *gameState) handleAI(w http.ResponseWriter, _ *http.Request) {
	g.mu.Lock()
	defer g.mu.Unlock()
	gameWon, _, draw := g.board.PlayerHasWon()
	if gameWon || draw {
		writeJSON(w, snapshot(g.board))
		return
	}
	options := g.board.MaxTakeBoards()
	if len(options) == 0 {
		options = g.board.AllMoveBoards()
	}
	if len(options) == 0 {
		writeJSON(w, snapshot(g.board))
		return
	}
	var best board.BoardState
	var bestVal float32
	for i, b := range options {
		b.SwapTeam()
		val := b.MinMax(0, -board.AlphaBetaMax, board.AlphaBetaMax, board.NewTable())
		if i == 0 || (g.board.Turn == board.White && val > bestVal) || (g.board.Turn == board.Black && val < bestVal) {
			best, bestVal = b, val
		}
	}
	_ = bestVal
	g.board = best
	writeJSON(w, snapshot(g.board))
}

func snapshot(bs board.BoardState) stateResponse {
	gameOver, winner, draw := bs.PlayerHasWon()
	moveMap, takeMap := legalMoveMap(&bs)
	res := stateResponse{Board: toRows(bs), Turn: map[board.TileTeam]string{board.White: "white", board.Black: "black"}[bs.Turn], GameOver: gameOver || draw, Draw: draw, Winner: "", ValidMoves: moveMap, IsTakeMap: takeMap, Depth: int(board.MaxDepth)}
	if gameOver {
		res.Winner = map[board.TileTeam]string{board.White: "white", board.Black: "black"}[winner]
	}
	return res
}

func legalMoveMap(bs *board.BoardState) (map[int][]int, bool) {
	takes := validUiTakes(bs, -1, [2]int{0, 0})
	if len(takes) > 0 {
		return takes, true
	}
	return validUiMoves(bs), false
}
func toRows(bs board.BoardState) []string {
	out := []string{}
	for y := 0; y < 8; y++ {
		row := ""
		for x := 0; x < 8; x++ {
			t, _ := bs.GetBoardTile(x, y)
			ch := '-'
			if t.Full == board.Filled {
				if t.Team == board.White {
					ch = 'w'
					if t.King == board.King {
						ch = 'W'
					}
				} else {
					ch = 'b'
					if t.King == board.King {
						ch = 'B'
					}
				}
			}
			row += string(ch)
		}
		out = append(out, row)
	}
	return out
}
func writeJSON(w http.ResponseWriter, v interface{}) {
	w.Header().Set("Content-Type", "application/json")
	_ = json.NewEncoder(w).Encode(v)
}
func contains(items []int, n int) bool {
	for _, v := range items {
		if v == n {
			return true
		}
	}
	return false
}

func validUiTakes(bs *board.BoardState, forcedIndex int, lastDir [2]int) map[int][]int {
	bestTake := 1
	valid := map[int][]int{}
	for i := 0; i < 64; i++ {
		if forcedIndex != -1 {
			i = forcedIndex
		}
		p, _ := bs.GetBoardTile(i%8, i/8)
		if p.Full == board.Empty || p.Team != bs.Turn {
			continue
		}
		var takes int
		var pos []int
		if p.King == board.King {
			takes, _, pos = bs.FindKingTakes(i%8, i/8, 0, lastDir)
		} else {
			takes, _, pos = bs.FindPawnTakes(i%8, i/8, 0)
		}
		if takes > bestTake {
			bestTake = takes
			valid = map[int][]int{i: pos}
		} else if takes == bestTake {
			valid[i] = pos
		}
		if forcedIndex != -1 {
			break
		}
	}
	return valid
}
func validUiMoves(bs *board.BoardState) map[int][]int {
	valid := map[int][]int{}
	for a := 0; a < 64; a++ {
		p, _ := bs.GetBoardTile(a%8, a/8)
		if p.Full == board.Empty || p.Team != bs.Turn {
			continue
		}
		stepMax := 1
		if p.King == board.King {
			stepMax = 7
		}
		x, y := a%8, a/8
		list := []int{}
		for _, d := range [4][2]int{{0, 1}, {0, -1}, {-1, 0}, {1, 0}} {
			for b := 1; b <= stepMax; b++ {
				if p.King == board.Pawn {
					if p.Team == board.White && d[0] == 0 && d[1] == 1 {
						continue
					}
					if p.Team == board.Black && d[0] == 0 && d[1] == -1 {
						continue
					}
				}
				mx, my := x+d[0]*b, y+d[1]*b
				t, on := bs.GetBoardTile(mx, my)
				if on && t.Full == board.Empty {
					list = append(list, my*8+mx)
				} else {
					break
				}
			}
		}
		if len(list) > 0 {
			valid[a] = list
		}
	}
	return valid
}
func tryMove(b *board.BoardState, fromIndex, toIndex int) (bool, [2]int) {
	tile, _ := b.GetBoardTile(fromIndex%8, fromIndex/8)
	b.SetBoardTile(toIndex%8, toIndex/8, tile)
	b.SetBoardTile(fromIndex%8, fromIndex/8, board.Tile{})
	changeIndex := toIndex - fromIndex
	change := 1
	if changeIndex >= 8 || changeIndex <= -8 {
		change = 8
	}
	if changeIndex < 0 {
		change *= -1
	}
	swap := true
	for i := fromIndex; i != toIndex; i += change {
		t, _ := b.GetBoardTile(i%8, i/8)
		if t.Full == board.Filled && i != fromIndex {
			b.SetBoardTile(i%8, i/8, board.Tile{})
			swap = false
		}
	}
	return swap, [2]int{(toIndex % 8) - (fromIndex % 8), (toIndex / 8) - (fromIndex / 8)}
}
