package main

import (
	"fmt"
	"math"
	"sort"
	"time"

	board "TurkishDraughts/Board"
)

const (
	TimeLimit      = 3 * time.Second
	MaxDepthLimit  = 64
	Infinity      = 1_000_000.0
	WinScore      = 900_000.0
	TimeoutMargin = 5 * time.Millisecond

	// Checking the clock at every node is surprisingly expensive.
	// We check it every N nodes instead.
	TimeCheckInterval = 4096
)

type TTFlag uint8

const (
	TTExact TTFlag = iota
	TTLowerBound
	TTUpperBound
)

type TTEntry struct {
	Depth int
	Value float64
	Flag  TTFlag
	Best  board.BoardState
	HasBest bool
}

type SearchStats struct {
	Nodes          int64
	QNodes         int64
	TTHits         int64
	Cutoffs        int64
	MoveCacheHits int64
	TakeCacheHits int64
	DepthDone      int
	TimedOut       bool
}

type Searcher struct {
	deadline time.Time
	table    map[board.BoardState]TTEntry
	stats    SearchStats

	// Search heuristics.
	// Killer moves: good quiet moves that caused beta cutoffs at a given ply.
	killers [MaxDepthLimit + 8][2]board.BoardState

	// History heuristic: successful move signatures become more preferred later.
	history map[MoveKey]int

	// Cached move generation. In this project ValidPlays() returns BoardState results,
	// and it is one of the most expensive repeated calls during search.
	movesCache map[board.BoardState][]board.BoardState
	takesCache map[board.BoardState][]board.BoardState
	countCache map[board.BoardState]int
	evalCache  map[board.BoardState]float64
}

type MoveKey struct {
	FromX int
	FromY int
	ToX   int
	ToY   int
	Team  board.TileTeam
}

type BestMoveResult struct {
	Board board.BoardState
	Score float64
	OK    bool
	Depth int
	Stats SearchStats
}

func main() {
	b := board.CreateStartingBoard()

	fmt.Println("Starting board:")
	b.Print()

	fmt.Println()
	fmt.Println("Thinking...")

	result := BestMoveTimed(b, TimeLimit)

	if !result.OK {
		fmt.Println("No legal moves found.")
		return
	}

	fmt.Println()
	fmt.Println("Best move result board:")
	result.Board.Print()

	PrintMoveDiff(b, result.Board)

	moves := b.LegalMovesDetailed()

	fmt.Println()
	fmt.Println("Legal detailed moves:", len(moves))

	for i, move := range moves {
		if i >= 5 {
			break
		}

		fmt.Printf(
			"Move %d: from (%d,%d) to (%d,%d), captures=%d, promote=%v\n",
			i+1,
			move.From.X,
			move.From.Y,
			move.To.X,
			move.To.Y,
			len(move.Captures),
			move.Promote,
		)
	}

	fmt.Println()
	fmt.Printf("Evaluation: %.3f\n", result.Score)
	fmt.Println("Completed depth:", result.Depth)
	fmt.Println("Time limit:", TimeLimit)
	fmt.Println("Nodes:", result.Stats.Nodes)
	fmt.Println("QNodes:", result.Stats.QNodes)
	fmt.Println("TT hits:", result.Stats.TTHits)
	fmt.Println("Move cache hits:", result.Stats.MoveCacheHits)
	fmt.Println("Take cache hits:", result.Stats.TakeCacheHits)
	fmt.Println("Cutoffs:", result.Stats.Cutoffs)
	fmt.Println("Timed out:", result.Stats.TimedOut)
}

// BestMoveTimed uses iterative deepening.
// It always returns the best move from the last fully completed depth.
func BestMoveTimed(bs board.BoardState, limit time.Duration) BestMoveResult {
	searcher := &Searcher{
		deadline: time.Now().Add(limit - TimeoutMargin),
		table:    make(map[board.BoardState]TTEntry, 1<<20),
		history:  make(map[MoveKey]int, 1<<14),
		movesCache: make(map[board.BoardState][]board.BoardState, 1<<18),
		takesCache: make(map[board.BoardState][]board.BoardState, 1<<16),
		countCache: make(map[board.BoardState]int, 1<<18),
		evalCache:  make(map[board.BoardState]float64, 1<<18),
	}

	options := searcher.validPlays(bs)
	if len(options) == 0 {
		return BestMoveResult{OK: false}
	}

	var best board.BoardState
	var bestScore float64
	var completedDepth int
	ok := false

	// Smart fallback: score legal moves quickly in case the time limit is extremely small.
	ordered := searcher.orderMoves(bs, options, nil)
	if len(ordered) > 0 {
		best = ordered[0]
		bestScore = searcher.evaluateAfterMove(bs, best)
		ok = true
	}

	for depth := 1; depth <= MaxDepthLimit; depth++ {
		if searcher.timeUpStrict() {
			searcher.stats.TimedOut = true
			break
		}

		// Aspiration window: after a previous completed score, search around it.
		// This usually creates more cutoffs and reaches deeper depths.
		window := 80.0
		alpha := -Infinity
		beta := Infinity

		if depth >= 3 && ok {
			alpha = bestScore - window
			beta = bestScore + window
		}

		score, move, finished := searcher.searchRootWindow(bs, depth, alpha, beta)

		// If the score falls outside the aspiration window, re-search full window.
		if finished && (score <= alpha || score >= beta) {
			score, move, finished = searcher.searchRootWindow(bs, depth, -Infinity, Infinity)
		}

		if !finished {
			searcher.stats.TimedOut = true
			break
		}

		bestScore = score
		best = move
		completedDepth = depth
		ok = true
		searcher.stats.DepthDone = depth
	}

	return BestMoveResult{
		Board: best,
		Score: bestScore,
		OK: ok,
		Depth: completedDepth,
		Stats: searcher.stats,
	}
}

func (s *Searcher) searchRootWindow(bs board.BoardState, depth int, alpha, beta float64) (float64, board.BoardState, bool) {
	options := s.validPlays(bs)
	if len(options) == 0 {
		if bs.Turn == board.White {
			return -WinScore, board.BoardState{}, true
		}
		return WinScore, board.BoardState{}, true
	}

	var ttBest *board.BoardState
	if entry, ok := s.table[bs]; ok && entry.HasBest {
		ttBest = &entry.Best
	}
	options = s.orderMoves(bs, options, ttBest)

	var best board.BoardState
	var bestScore float64

	if bs.Turn == board.White {
		bestScore = -Infinity
		for _, option := range options {
			child := option
			child.SwapTeam()

			score, finished := s.alphaBeta(child, depth-1, 1, alpha, beta)
			if !finished {
				return bestScore, best, false
			}

			if score > bestScore {
				bestScore = score
				best = option
			}
			if score > alpha {
				alpha = score
			}
		}
	} else {
		bestScore = Infinity
		for _, option := range options {
			child := option
			child.SwapTeam()

			score, finished := s.alphaBeta(child, depth-1, 1, alpha, beta)
			if !finished {
				return bestScore, best, false
			}

			if score < bestScore {
				bestScore = score
				best = option
			}
			if score < beta {
				beta = score
			}
		}
	}

	s.table[bs] = TTEntry{
		Depth: depth,
		Value: bestScore,
		Flag: TTExact,
		Best: best,
		HasBest: true,
	}

	return bestScore, best, true
}

func (s *Searcher) alphaBeta(bs board.BoardState, depth int, ply int, alpha, beta float64) (float64, bool) {
	if s.timeUp() {
		return 0, false
	}

	s.stats.Nodes++

	if won, winner, draw := bs.PlayerHasWon(); won || draw {
		if draw {
			return 0, true
		}
		if winner == board.White {
			return WinScore + float64(depth), true
		}
		return -WinScore - float64(depth), true
	}

	options := s.validPlays(bs)
	if len(options) == 0 {
		if bs.Turn == board.White {
			return -WinScore - float64(depth), true
		}
		return WinScore + float64(depth), true
	}

	// Tactical extension at horizon, but keep it short.
	// Earlier V2 used 6; that created too many QNodes in tactical branches.
	if depth <= 0 {
		captures := s.maxTakeBoards(bs)
		if len(captures) == 0 {
			return s.evaluateCached(bs), true
		}
		return s.captureSearch(bs, alpha, beta, 4)
	}

	alphaOrig := alpha
	betaOrig := beta

	if entry, ok := s.table[bs]; ok && entry.Depth >= depth {
		s.stats.TTHits++
		switch entry.Flag {
		case TTExact:
			return entry.Value, true
		case TTLowerBound:
			if entry.Value > alpha {
				alpha = entry.Value
			}
		case TTUpperBound:
			if entry.Value < beta {
				beta = entry.Value
			}
		}
		if alpha >= beta {
			return entry.Value, true
		}
	}

	var ttBest *board.BoardState
	if entry, ok := s.table[bs]; ok && entry.HasBest {
		ttBest = &entry.Best
	}
	options = s.orderMovesAtPly(bs, options, ttBest, ply)

	var bestMove board.BoardState
	hasBest := false
	var bestScore float64

	if bs.Turn == board.White {
		bestScore = -Infinity

		for i, option := range options {
			child := option
			child.SwapTeam()

			reduction := s.lateMoveReduction(bs, option, depth, ply, i)
			searchDepth := depth - 1 - reduction
			if searchDepth < 0 {
				searchDepth = 0
			}

			var score float64
			var finished bool

			// Principal Variation Search:
			// First move gets a full window. Later moves first try a null window.
			if i == 0 {
				score, finished = s.alphaBeta(child, searchDepth, ply+1, alpha, beta)
			} else {
				score, finished = s.alphaBeta(child, searchDepth, ply+1, alpha, alpha+1)
				if finished && score > alpha && score < beta {
					// If the null-window search proves interesting, re-search full window.
					score, finished = s.alphaBeta(child, depth-1, ply+1, alpha, beta)
				}
			}

			if !finished {
				return 0, false
			}

			// If a reduced move beats alpha, verify at full depth.
			if reduction > 0 && score > alpha {
				score, finished = s.alphaBeta(child, depth-1, ply+1, alpha, beta)
				if !finished {
					return 0, false
				}
			}

			if score > bestScore {
				bestScore = score
				bestMove = option
				hasBest = true
			}
			if score > alpha {
				alpha = score
			}
			if alpha >= beta {
				s.stats.Cutoffs++
				s.recordCutoff(bs, option, depth, ply)
				break
			}
		}
	} else {
		bestScore = Infinity

		for i, option := range options {
			child := option
			child.SwapTeam()

			reduction := s.lateMoveReduction(bs, option, depth, ply, i)
			searchDepth := depth - 1 - reduction
			if searchDepth < 0 {
				searchDepth = 0
			}

			var score float64
			var finished bool

			if i == 0 {
				score, finished = s.alphaBeta(child, searchDepth, ply+1, alpha, beta)
			} else {
				score, finished = s.alphaBeta(child, searchDepth, ply+1, beta-1, beta)
				if finished && score < beta && score > alpha {
					score, finished = s.alphaBeta(child, depth-1, ply+1, alpha, beta)
				}
			}

			if !finished {
				return 0, false
			}

			if reduction > 0 && score < beta {
				score, finished = s.alphaBeta(child, depth-1, ply+1, alpha, beta)
				if !finished {
					return 0, false
				}
			}

			if score < bestScore {
				bestScore = score
				bestMove = option
				hasBest = true
			}
			if score < beta {
				beta = score
			}
			if alpha >= beta {
				s.stats.Cutoffs++
				s.recordCutoff(bs, option, depth, ply)
				break
			}
		}
	}

	flag := TTExact
	if bestScore <= alphaOrig {
		flag = TTUpperBound
	} else if bestScore >= betaOrig {
		flag = TTLowerBound
	}

	s.table[bs] = TTEntry{
		Depth: depth,
		Value: bestScore,
		Flag: flag,
		Best: bestMove,
		HasBest: hasBest,
	}

	return bestScore, true
}

// captureSearch searches only forced capture continuations at the horizon.
// Turkish draughts is very tactical, so this prevents the engine from stopping
// in the middle of a capture sequence.
func (s *Searcher) captureSearch(bs board.BoardState, alpha, beta float64, remaining int) (float64, bool) {
	if s.timeUp() {
		return 0, false
	}

	s.stats.QNodes++

	if won, winner, draw := bs.PlayerHasWon(); won || draw {
		if draw {
			return 0, true
		}
		if winner == board.White {
			return WinScore, true
		}
		return -WinScore, true
	}

	if remaining <= 0 {
		return s.evaluateCached(bs), true
	}

	captures := s.maxTakeBoards(bs)
	if len(captures) == 0 {
		return s.evaluateCached(bs), true
	}

	captures = s.orderMoves(bs, captures, nil)

	if bs.Turn == board.White {
		best := -Infinity
		for _, option := range captures {
			child := option
			child.SwapTeam()

			score, finished := s.captureSearch(child, alpha, beta, remaining-1)
			if !finished {
				return 0, false
			}

			if score > best {
				best = score
			}
			if score > alpha {
				alpha = score
			}
			if alpha >= beta {
				s.stats.Cutoffs++
				break
			}
		}
		return best, true
	}

	best := Infinity
	for _, option := range captures {
		child := option
		child.SwapTeam()

		score, finished := s.captureSearch(child, alpha, beta, remaining-1)
		if !finished {
			return 0, false
		}

		if score < best {
			best = score
		}
		if score < beta {
			beta = score
		}
		if alpha >= beta {
			s.stats.Cutoffs++
			break
		}
	}
	return best, true
}

func (s *Searcher) validPlays(bs board.BoardState) []board.BoardState {
	if moves, ok := s.movesCache[bs]; ok {
		s.stats.MoveCacheHits++
		return moves
	}
	moves := bs.ValidPlays()
	s.movesCache[bs] = moves
	return moves
}

func (s *Searcher) maxTakeBoards(bs board.BoardState) []board.BoardState {
	if moves, ok := s.takesCache[bs]; ok {
		s.stats.TakeCacheHits++
		return moves
	}
	moves := bs.MaxTakeBoards()
	s.takesCache[bs] = moves
	return moves
}

func (s *Searcher) countPieces(bs board.BoardState) int {
	if n, ok := s.countCache[bs]; ok {
		return n
	}
	n := countPieces(bs)
	s.countCache[bs] = n
	return n
}

func (s *Searcher) evaluateCached(bs board.BoardState) float64 {
	if value, ok := s.evalCache[bs]; ok {
		return value
	}
	value := s.evaluateRaw(bs)
	s.evalCache[bs] = value
	return value
}

func (s *Searcher) orderMoves(before board.BoardState, moves []board.BoardState, ttBest *board.BoardState) []board.BoardState {
	ordered := make([]board.BoardState, len(moves))
	copy(ordered, moves)

	type scoredMove struct {
		move  board.BoardState
		score float64
	}
	scored := make([]scoredMove, 0, len(ordered))

	for _, mv := range ordered {
		score := s.moveOrderingScore(before, mv)

		if ttBest != nil && mv == *ttBest {
			score += 100_000
		}

		scored = append(scored, scoredMove{move: mv, score: score})
	}

	if before.Turn == board.White {
		sort.Slice(scored, func(i, j int) bool {
			return scored[i].score > scored[j].score
		})
	} else {
		sort.Slice(scored, func(i, j int) bool {
			return scored[i].score < scored[j].score
		})
	}

	for i := range scored {
		ordered[i] = scored[i].move
	}
	return ordered
}

func (s *Searcher) orderMovesAtPly(before board.BoardState, moves []board.BoardState, ttBest *board.BoardState, ply int) []board.BoardState {
	ordered := make([]board.BoardState, len(moves))
	copy(ordered, moves)

	type scoredMove struct {
		move  board.BoardState
		score float64
	}
	scored := make([]scoredMove, 0, len(ordered))

	for _, mv := range ordered {
		score := s.moveOrderingScore(before, mv)

		if ttBest != nil && mv == *ttBest {
			score += 300_000
		}

		if ply >= 0 && ply < len(s.killers) {
			if mv == s.killers[ply][0] {
				score += 80_000
			} else if mv == s.killers[ply][1] {
				score += 50_000
			}
		}

		if key, ok := moveKey(before, mv); ok {
			if before.Turn == board.White {
				score += float64(s.history[key])
			} else {
				score -= float64(s.history[key])
			}
		}

		scored = append(scored, scoredMove{move: mv, score: score})
	}

	if before.Turn == board.White {
		sort.Slice(scored, func(i, j int) bool {
			return scored[i].score > scored[j].score
		})
	} else {
		sort.Slice(scored, func(i, j int) bool {
			return scored[i].score < scored[j].score
		})
	}

	for i := range scored {
		ordered[i] = scored[i].move
	}
	return ordered
}

func (s *Searcher) moveOrderingScore(before, after board.BoardState) float64 {
	score := s.evaluateAfterMove(before, after)

	beforeCount := s.countPieces(before)
	afterCount := s.countPieces(after)
	captured := beforeCount - afterCount
	score += float64(captured) * 250.0

	from, to, hasMove := findMainMove(before, after)
	if hasMove {
		moved, _ := before.GetBoardTile(from[0], from[1])
		landed, _ := after.GetBoardTile(to[0], to[1])

		if moved.King == board.Pawn && landed.King == board.King {
			if moved.Team == board.White {
				score += 180
			} else {
				score -= 180
			}
		}

		// Prefer central landing squares slightly.
		center := centerBonus(to[0], to[1]) * 5.0
		if landed.Team == board.White {
			score += center
		} else {
			score -= center
		}
	}

	return score
}

func (s *Searcher) evaluateAfterMove(before, after board.BoardState) float64 {
	return s.evaluate(after)
}

func (s *Searcher) evaluateRaw(bs board.BoardState) float64 {
	if won, winner, draw := bs.PlayerHasWon(); won || draw {
		if draw {
			return 0
		}
		if winner == board.White {
			return WinScore
		}
		return -WinScore
	}

	var score float64
	whitePieces, blackPieces := 0, 0
	whiteKings, blackKings := 0, 0

	// Fast handcrafted evaluation.
	// Important: do NOT call ValidPlays() here, because evaluation runs at many leaf nodes.
	for y := 0; y < 8; y++ {
		for x := 0; x < 8; x++ {
			tile, _ := bs.GetBoardTile(x, y)
			if tile.Full == board.Empty {
				continue
			}

			if tile.Team == board.White {
				whitePieces++
				if tile.King == board.King {
					whiteKings++
					score += 560
					score += kingMobilityPotential(bs, x, y, board.White) * 7
				} else {
					score += 100
					score += float64(7-y) * 9
					score += promotionPressure(y, board.White)
				}

				score += centerBonus(x, y) * 5
				score += safetyBonus(bs, x, y, board.White)
				score -= isolationPenalty(bs, x, y, board.White)
				score += connectionBonus(bs, x, y, board.White)
				score += fastMovePotential(bs, x, y, board.White) * 2.2
				score += fastCaptureThreat(bs, x, y, board.White) * 28

				if x == 0 || x == 7 {
					score -= 4
				}
			} else {
				blackPieces++
				if tile.King == board.King {
					blackKings++
					score -= 560
					score -= kingMobilityPotential(bs, x, y, board.Black) * 7
				} else {
					score -= 100
					score -= float64(y) * 9
					score -= promotionPressure(y, board.Black)
				}

				score -= centerBonus(x, y) * 5
				score -= safetyBonus(bs, x, y, board.Black)
				score += isolationPenalty(bs, x, y, board.Black)
				score -= connectionBonus(bs, x, y, board.Black)
				score -= fastMovePotential(bs, x, y, board.Black) * 2.2
				score -= fastCaptureThreat(bs, x, y, board.Black) * 28

				if x == 0 || x == 7 {
					score += 4
				}
			}
		}
	}

	totalPieces := whitePieces + blackPieces

	// Endgame: kings, mobility potential, and promotion become more decisive.
	if totalPieces <= 8 {
		score += float64(whiteKings-blackKings) * 130
		score += fastTeamMovePotential(bs, board.White)*2.5 - fastTeamMovePotential(bs, board.Black)*2.5
	}

	// Avoid completely trading into bad material.
	score += float64(whitePieces-blackPieces) * 4

	// Tiny side-to-move tempo.
	if bs.Turn == board.White {
		score += 1.0
	} else {
		score -= 1.0
	}

	return score
}

func (s *Searcher) evaluate(bs board.BoardState) float64 {
	return s.evaluateCached(bs)
}


func mobility(bs board.BoardState, team board.TileTeam) int {
	copyBS := bs
	copyBS.Turn = team
	return len(copyBS.ValidPlays())
}

func captureCount(bs board.BoardState, team board.TileTeam) int {
	copyBS := bs
	copyBS.Turn = team
	return len(copyBS.MaxTakeBoards())
}

func countPieces(bs board.BoardState) int {
	count := 0
	for y := 0; y < 8; y++ {
		for x := 0; x < 8; x++ {
			tile, _ := bs.GetBoardTile(x, y)
			if tile.Full == board.Filled {
				count++
			}
		}
	}
	return count
}

func centerBonus(x, y int) float64 {
	dx := math.Abs(float64(x) - 3.5)
	dy := math.Abs(float64(y) - 3.5)
	return 7.0 - (dx + dy)
}

func promotionPressure(y int, team board.TileTeam) float64 {
	if team == board.White {
		switch y {
		case 1:
			return 55
		case 2:
			return 30
		case 3:
			return 14
		}
		return 0
	}

	switch y {
	case 6:
		return 55
	case 5:
		return 30
	case 4:
		return 14
	}
	return 0
}

func safetyBonus(bs board.BoardState, x, y int, team board.TileTeam) float64 {
	bonus := 0.0

	// Edge pieces are harder to capture, but do not overvalue them.
	if x == 0 || x == 7 || y == 0 || y == 7 {
		bonus += 10
	}

	// Friendly neighboring orthogonal pieces support the piece.
	for _, d := range [][2]int{{0, 1}, {0, -1}, {1, 0}, {-1, 0}} {
		t, ok := bs.GetBoardTile(x+d[0], y+d[1])
		if ok && t.Full == board.Filled && t.Team == team {
			bonus += 7
		}
	}

	return bonus
}

func connectionBonus(bs board.BoardState, x, y int, team board.TileTeam) float64 {
	bonus := 0.0
	for _, d := range [][2]int{{1, 0}, {-1, 0}, {0, 1}, {0, -1}} {
		t, ok := bs.GetBoardTile(x+d[0], y+d[1])
		if ok && t.Full == board.Filled && t.Team == team {
			bonus += 4
		}
	}
	return bonus
}

func isolationPenalty(bs board.BoardState, x, y int, team board.TileTeam) float64 {
	for _, d := range [][2]int{
		{1, 0}, {-1, 0}, {0, 1}, {0, -1},
		{1, 1}, {-1, -1}, {1, -1}, {-1, 1},
	} {
		t, ok := bs.GetBoardTile(x+d[0], y+d[1])
		if ok && t.Full == board.Filled && t.Team == team {
			return 0
		}
	}
	return 12
}

func kingMobilityPotential(bs board.BoardState, x, y int, team board.TileTeam) float64 {
	total := 0.0
	for _, d := range [][2]int{{0, 1}, {0, -1}, {1, 0}, {-1, 0}} {
		for step := 1; step < 8; step++ {
			t, ok := bs.GetBoardTile(x+d[0]*step, y+d[1]*step)
			if !ok || t.Full == board.Filled {
				break
			}
			total++
		}
	}
	return total
}

func (s *Searcher) lateMoveReduction(before, after board.BoardState, depth int, ply int, moveIndex int) int {
	if depth < 4 || moveIndex < 4 {
		return 0
	}

	// Do not reduce tactical moves, promotions, or TT/killer-like moves.
	beforeCount := s.countPieces(before)
	afterCount := s.countPieces(after)
	if beforeCount != afterCount {
		return 0
	}

	from, to, ok := findMainMove(before, after)
	if !ok {
		return 0
	}

	moved, _ := before.GetBoardTile(from[0], from[1])
	landed, _ := after.GetBoardTile(to[0], to[1])
	if moved.King == board.Pawn && landed.King == board.King {
		return 0
	}

	if ply >= 0 && ply < len(s.killers) {
		if after == s.killers[ply][0] || after == s.killers[ply][1] {
			return 0
		}
	}

	if depth >= 7 && moveIndex >= 10 {
		return 2
	}
	return 1
}

func (s *Searcher) recordCutoff(before, move board.BoardState, depth int, ply int) {
	// Captures are already prioritized; killer/history is most useful for quiet cutoffs.
	if s.countPieces(before) != s.countPieces(move) {
		return
	}

	if ply >= 0 && ply < len(s.killers) {
		if move != s.killers[ply][0] {
			s.killers[ply][1] = s.killers[ply][0]
			s.killers[ply][0] = move
		}
	}

	if key, ok := moveKey(before, move); ok {
		s.history[key] += depth * depth
		if s.history[key] > 1_000_000 {
			for k, v := range s.history {
				s.history[k] = v / 2
			}
		}
	}
}

func moveKey(before, after board.BoardState) (MoveKey, bool) {
	from, to, ok := findMainMove(before, after)
	if !ok {
		return MoveKey{}, false
	}

	tile, _ := before.GetBoardTile(from[0], from[1])
	if tile.Full == board.Empty {
		return MoveKey{}, false
	}

	return MoveKey{
		FromX: from[0],
		FromY: from[1],
		ToX:   to[0],
		ToY:   to[1],
		Team:  tile.Team,
	}, true
}

func fastTeamMovePotential(bs board.BoardState, team board.TileTeam) float64 {
	total := 0.0
	for y := 0; y < 8; y++ {
		for x := 0; x < 8; x++ {
			tile, _ := bs.GetBoardTile(x, y)
			if tile.Full == board.Filled && tile.Team == team {
				total += fastMovePotential(bs, x, y, team)
			}
		}
	}
	return total
}

func fastMovePotential(bs board.BoardState, x, y int, team board.TileTeam) float64 {
	tile, _ := bs.GetBoardTile(x, y)
	if tile.Full == board.Empty {
		return 0
	}

	if tile.King == board.King {
		return kingMobilityPotential(bs, x, y, team)
	}

	total := 0.0
	dirs := [][2]int{{1, 0}, {-1, 0}}

	if team == board.White {
		dirs = append(dirs, [2]int{0, -1})
	} else {
		dirs = append(dirs, [2]int{0, 1})
	}

	for _, d := range dirs {
		t, ok := bs.GetBoardTile(x+d[0], y+d[1])
		if ok && t.Full == board.Empty {
			total++
		}
	}

	return total
}

func fastCaptureThreat(bs board.BoardState, x, y int, team board.TileTeam) float64 {
	tile, _ := bs.GetBoardTile(x, y)
	if tile.Full == board.Empty {
		return 0
	}

	if tile.King == board.King {
		return fastKingCaptureThreat(bs, x, y, team)
	}

	total := 0.0
	for _, d := range [][2]int{{1, 0}, {-1, 0}, {0, 1}, {0, -1}} {
		mid, okMid := bs.GetBoardTile(x+d[0], y+d[1])
		land, okLand := bs.GetBoardTile(x+d[0]*2, y+d[1]*2)

		if okMid && okLand &&
			mid.Full == board.Filled &&
			mid.Team != team &&
			land.Full == board.Empty {
			total++
		}
	}
	return total
}

func fastKingCaptureThreat(bs board.BoardState, x, y int, team board.TileTeam) float64 {
	total := 0.0
	for _, d := range [][2]int{{1, 0}, {-1, 0}, {0, 1}, {0, -1}} {
		seenEnemy := false
		for step := 1; step < 8; step++ {
			t, ok := bs.GetBoardTile(x+d[0]*step, y+d[1]*step)
			if !ok {
				break
			}
			if t.Full == board.Empty {
				if seenEnemy {
					total++
					break
				}
				continue
			}
			if t.Team == team {
				break
			}
			if seenEnemy {
				break
			}
			seenEnemy = true
		}
	}
	return total
}

func findMainMove(before, after board.BoardState) ([2]int, [2]int, bool) {
	var removed [][2]int
	var added [][2]int

	for y := 0; y < 8; y++ {
		for x := 0; x < 8; x++ {
			bt, _ := before.GetBoardTile(x, y)
			at, _ := after.GetBoardTile(x, y)

			if bt.Full == board.Filled && at.Full == board.Empty {
				removed = append(removed, [2]int{x, y})
			}
			if bt.Full == board.Empty && at.Full == board.Filled {
				added = append(added, [2]int{x, y})
			}
		}
	}

	if len(added) == 0 || len(removed) == 0 {
		return [2]int{}, [2]int{}, false
	}

	// In normal moves, removed has one item. In captures, removed includes captured pieces too.
	// Pick the removed square whose piece has the same team as the added piece.
	to := added[0]
	landed, _ := after.GetBoardTile(to[0], to[1])

	for _, r := range removed {
		moved, _ := before.GetBoardTile(r[0], r[1])
		if moved.Full == board.Filled && moved.Team == landed.Team {
			return r, to, true
		}
	}

	return removed[0], to, true
}

func (s *Searcher) timeUp() bool {
	// s.stats.Nodes is already incremented in alphaBeta.
	// In captureSearch, QNodes is incremented, so include both.
	visited := s.stats.Nodes + s.stats.QNodes
	if visited%TimeCheckInterval != 0 {
		return false
	}
	return time.Now().After(s.deadline)
}

func (s *Searcher) timeUpStrict() bool {
	return time.Now().After(s.deadline)
}

func PrintMoveDiff(before board.BoardState, after board.BoardState) {
	fmt.Println()
	fmt.Println("Move diff:")

	beforePieces := extractPieces(before)
	afterPieces := extractPieces(after)

	for pos, piece := range beforePieces {
		if _, exists := afterPieces[pos]; !exists {
			fmt.Printf("Removed from %s: %s\n", pos, piece)
		}
	}

	for pos, piece := range afterPieces {
		if _, exists := beforePieces[pos]; !exists {
			fmt.Printf("Added to %s: %s\n", pos, piece)
		}
	}
}

func extractPieces(bs board.BoardState) map[string]string {
	result := make(map[string]string)

	for y := 0; y < 8; y++ {
		for x := 0; x < 8; x++ {
			tile, _ := bs.GetBoardTile(x, y)

			if tile.Full == board.Empty {
				continue
			}

			key := fmt.Sprintf("%d,%d", x, y)
			result[key] = tileToString(tile)
		}
	}

	return result
}

func tileToString(tile board.Tile) string {
	if tile.Team == board.White {
		if tile.King == board.King {
			return "WhiteKing"
		}
		return "WhitePawn"
	}

	if tile.King == board.King {
		return "BlackKing"
	}

	return "BlackPawn"
}

func FindDetailedMove(before board.BoardState, after board.BoardState) (board.Move, bool) {
	moves := before.LegalMovesDetailed()

	for _, move := range moves {
		if move.Result == after {
			return move, true
		}
	}

	return board.Move{}, false
}