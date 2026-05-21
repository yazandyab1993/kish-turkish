package board

type Square struct {
	X int
	Y int
}

type Move struct {
	From      Square
	To        Square
	Captures  []Square
	IsCapture bool
	Promote   bool
	Result    BoardState
}

// LegalMovesDetailed returns legal moves as real Move objects.
// It preserves the original engine semantics:
// - if captures exist, only maximum-capture sequences are legal
// - otherwise quiet moves are returned
// - promotion is marked on Move.Promote, but Result is left unpromoted;
//   the existing search calls Result.SwapTeam(), which performs promotion.
func (bs *BoardState) LegalMovesDetailed() []Move {
	captures := bs.CaptureMovesDetailed()
	if len(captures) > 0 {
		return captures
	}

	return bs.QuietMovesDetailed()
}

func (bs *BoardState) QuietMovesDetailed() []Move {
	moves := []Move{}

	for i := 0; i < 64; i++ {
		x := i % 8
		y := i / 8

		piece, _ := bs.GetBoardTile(x, y)
		if piece.Full == Empty || piece.Team != bs.Turn {
			continue
		}

		if piece.King == King {
			moves = append(moves, bs.quietKingMovesDetailed(x, y)...)
		} else {
			moves = append(moves, bs.quietPawnMovesDetailed(x, y)...)
		}
	}

	return moves
}

func (bs *BoardState) quietPawnMovesDetailed(x, y int) []Move {
	moves := []Move{}
	piece, _ := bs.GetBoardTile(x, y)

	for _, d := range [][2]int{{0, 1}, {0, -1}, {-1, 0}, {1, 0}} {
		// Preserve original project rules:
		// White pawns do not move down, black pawns do not move up.
		if piece.Team == White && d[1] == 1 {
			continue
		}
		if piece.Team == Black && d[1] == -1 {
			continue
		}

		toX := x + d[0]
		toY := y + d[1]
		target, ok := bs.GetBoardTile(toX, toY)

		if !ok || target.Full != Empty {
			continue
		}

		result := *bs
		result.SetBoardTile(toX, toY, piece)
		result.SetBoardTile(x, y, Tile{})

		moves = append(moves, Move{
			From:    Square{X: x, Y: y},
			To:      Square{X: toX, Y: toY},
			Promote: wouldPromote(piece, toY),
			Result:  result,
		})
	}

	return moves
}

func (bs *BoardState) quietKingMovesDetailed(x, y int) []Move {
	moves := []Move{}
	piece, _ := bs.GetBoardTile(x, y)

	for _, d := range [][2]int{{0, 1}, {0, -1}, {-1, 0}, {1, 0}} {
		for step := 1; step < 8; step++ {
			toX := x + d[0]*step
			toY := y + d[1]*step
			target, ok := bs.GetBoardTile(toX, toY)

			if !ok || target.Full != Empty {
				break
			}

			result := *bs
			result.SetBoardTile(toX, toY, piece)
			result.SetBoardTile(x, y, Tile{})

			moves = append(moves, Move{
				From:   Square{X: x, Y: y},
				To:     Square{X: toX, Y: toY},
				Result: result,
			})
		}
	}

	return moves
}

func (bs *BoardState) CaptureMovesDetailed() []Move {
	bestCount := 0
	bestMoves := []Move{}

	for i := 0; i < 64; i++ {
		x := i % 8
		y := i / 8

		piece, _ := bs.GetBoardTile(x, y)
		if piece.Full == Empty || piece.Team != bs.Turn {
			continue
		}

		var pieceMoves []Move
		if piece.King == King {
			pieceMoves = bs.kingCaptureSequencesDetailed(
				Square{X: x, Y: y},
				Square{X: x, Y: y},
				nil,
				[2]int{0, 0},
			)
		} else {
			pieceMoves = bs.pawnCaptureSequencesDetailed(
				Square{X: x, Y: y},
				Square{X: x, Y: y},
				nil,
			)
		}

		for _, move := range pieceMoves {
			count := len(move.Captures)
			if count == 0 {
				continue
			}

			if count > bestCount {
				bestCount = count
				bestMoves = []Move{move}
			} else if count == bestCount {
				bestMoves = append(bestMoves, move)
			}
		}
	}

	return removeDuplicateMoves(bestMoves)
}

func (bs *BoardState) pawnCaptureSequencesDetailed(origin Square, current Square, captures []Square) []Move {
	piece, _ := bs.GetBoardTile(current.X, current.Y)
	results := []Move{}
	found := false

	for _, d := range [][2]int{{0, 1}, {0, -1}, {-1, 0}, {1, 0}} {
		// Preserve original project rules:
		// White pawns cannot capture downward, black pawns cannot capture upward.
		if piece.Team == White && d[0] == 0 && d[1] == 1 {
			continue
		}
		if piece.Team == Black && d[0] == 0 && d[1] == -1 {
			continue
		}

		jumpX := current.X + d[0]
		jumpY := current.Y + d[1]
		landX := current.X + 2*d[0]
		landY := current.Y + 2*d[1]

		jumpTile, okJump := bs.GetBoardTile(jumpX, jumpY)
		landTile, okLand := bs.GetBoardTile(landX, landY)

		if !okJump || !okLand {
			continue
		}

		if jumpTile.Full != Filled || jumpTile.Team == piece.Team || landTile.Full != Empty {
			continue
		}

		found = true

		next := *bs
		next.SetBoardTile(landX, landY, piece)
		next.SetBoardTile(jumpX, jumpY, Tile{})
		next.SetBoardTile(current.X, current.Y, Tile{})

		nextCaptures := appendSquare(captures, Square{X: jumpX, Y: jumpY})
		nextMoves := next.pawnCaptureSequencesDetailed(
			origin,
			Square{X: landX, Y: landY},
			nextCaptures,
		)

		results = append(results, nextMoves...)
	}

	if !found && len(captures) > 0 {
		results = append(results, Move{
			From:      origin,
			To:        current,
			Captures:  cloneSquares(captures),
			IsCapture: true,
			Promote:   wouldPromote(piece, current.Y),
			Result:    *bs,
		})
	}

	return results
}

func (bs *BoardState) kingCaptureSequencesDetailed(origin Square, current Square, captures []Square, lastDir [2]int) []Move {
	piece, _ := bs.GetBoardTile(current.X, current.Y)
	results := []Move{}
	found := false

	for _, d := range [][2]int{{0, 1}, {0, -1}, {-1, 0}, {1, 0}} {
		// Preserve original project rule:
		// A king cannot immediately reverse direction during a capture chain.
		if -lastDir[0] == d[0] && -lastDir[1] == d[1] {
			continue
		}

		jumpX, jumpY := 0, 0
		i := 1
		exit := false
		foundEnemy := false

		for i < 8 {
			x := current.X + d[0]*i
			y := current.Y + d[1]*i
			tile, ok := bs.GetBoardTile(x, y)
			i++

			if !ok {
				exit = true
				break
			}

			if tile.Full == Empty {
				continue
			}

			if tile.Team == piece.Team {
				exit = true
			} else {
				jumpX = x
				jumpY = y
				foundEnemy = true
			}
			break
		}

		if exit || !foundEnemy {
			continue
		}

		for i < 8 {
			landX := current.X + d[0]*i
			landY := current.Y + d[1]*i
			landTile, ok := bs.GetBoardTile(landX, landY)
			i++

			if !ok || landTile.Full == Filled {
				break
			}

			found = true

			next := *bs
			next.SetBoardTile(landX, landY, piece)
			next.SetBoardTile(jumpX, jumpY, Tile{})
			next.SetBoardTile(current.X, current.Y, Tile{})

			nextCaptures := appendSquare(captures, Square{X: jumpX, Y: jumpY})
			nextMoves := next.kingCaptureSequencesDetailed(
				origin,
				Square{X: landX, Y: landY},
				nextCaptures,
				d,
			)

			results = append(results, nextMoves...)
		}
	}

	if !found && len(captures) > 0 {
		results = append(results, Move{
			From:      origin,
			To:        current,
			Captures:  cloneSquares(captures),
			IsCapture: true,
			Result:    *bs,
		})
	}

	return results
}

func wouldPromote(piece Tile, y int) bool {
	if piece.King == King {
		return false
	}

	return (piece.Team == White && y == 0) || (piece.Team == Black && y == 7)
}

func appendSquare(items []Square, item Square) []Square {
	next := make([]Square, len(items), len(items)+1)
	copy(next, items)
	next = append(next, item)
	return next
}

func cloneSquares(items []Square) []Square {
	next := make([]Square, len(items))
	copy(next, items)
	return next
}

func removeDuplicateMoves(moves []Move) []Move {
	seen := make(map[BoardState]bool)
	result := []Move{}

	for _, move := range moves {
		if seen[move.Result] {
			continue
		}
		seen[move.Result] = true
		result = append(result, move)
	}

	return result
}
