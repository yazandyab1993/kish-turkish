package main

import (
	"fmt"
	"time"

	board "TurkishDraughts/Board"
)

func main() {
	b := board.CreateStartingBoard()

	fmt.Println("=== Turkish Dama Move Generator Benchmark ===")
	fmt.Println()

	fmt.Println("Starting board:")
	b.Print()
	fmt.Println()

	compareGenerators("Starting position", b)
	benchmarkPosition("Starting position", b, 200000)

	fmt.Println()
	fmt.Println("Generating sample positions from original ValidPlays()...")
	positions := generateSamplePositions(b, 5)

	for i, pos := range positions {
		fmt.Println()
		fmt.Printf("=== Sample position %d ===\n", i+1)
		pos.Print()
		compareGenerators(fmt.Sprintf("Sample position %d", i+1), pos)
		benchmarkPosition(fmt.Sprintf("Sample position %d", i+1), pos, 50000)
	}
}

func compareGenerators(name string, bs board.BoardState) {
	oldBoards := bs.ValidPlays()
	newMoves := bs.LegalMovesDetailed()

	oldSet := make(map[board.BoardState]bool)
	newSet := make(map[board.BoardState]bool)

	for _, result := range oldBoards {
		oldSet[result] = true
	}

	for _, move := range newMoves {
		newSet[move.Result] = true
	}

	missingFromNew := 0
	extraInNew := 0

	for result := range oldSet {
		if !newSet[result] {
			missingFromNew++
		}
	}

	for result := range newSet {
		if !oldSet[result] {
			extraInNew++
		}
	}

	fmt.Println("Compare:", name)
	fmt.Println("Original ValidPlays count:", len(oldBoards))
	fmt.Println("Detailed LegalMoves count:", len(newMoves))
	fmt.Println("Unique original boards:", len(oldSet))
	fmt.Println("Unique detailed boards:", len(newSet))
	fmt.Println("Missing from detailed:", missingFromNew)
	fmt.Println("Extra in detailed:", extraInNew)

	if missingFromNew == 0 && extraInNew == 0 {
		fmt.Println("Status: OK - same resulting boards")
	} else {
		fmt.Println("Status: MISMATCH - generators differ")
	}
}

func benchmarkPosition(name string, bs board.BoardState, iterations int) {
	fmt.Println()
	fmt.Println("Benchmark:", name)
	fmt.Println("Iterations:", iterations)

	startOld := time.Now()
	oldTotal := 0
	for i := 0; i < iterations; i++ {
		oldTotal += len(bs.ValidPlays())
	}
	oldElapsed := time.Since(startOld)

	startNew := time.Now()
	newTotal := 0
	captures := 0
	promotions := 0

	for i := 0; i < iterations; i++ {
		moves := bs.LegalMovesDetailed()
		newTotal += len(moves)

		for _, move := range moves {
			if move.IsCapture {
				captures++
			}
			if move.Promote {
				promotions++
			}
		}
	}
	newElapsed := time.Since(startNew)

	fmt.Println("Original total moves:", oldTotal)
	fmt.Println("Detailed total moves:", newTotal)
	fmt.Println("Detailed captures seen:", captures)
	fmt.Println("Detailed promotions seen:", promotions)
	fmt.Println("Original time:", oldElapsed)
	fmt.Println("Detailed time:", newElapsed)

	if newElapsed > 0 {
		ratio := float64(oldElapsed) / float64(newElapsed)
		fmt.Printf("Speed ratio old/new: %.3fx\n", ratio)
	}

	if oldElapsed > 0 {
		ratio := float64(newElapsed) / float64(oldElapsed)
		fmt.Printf("Detailed cost vs original: %.3fx\n", ratio)
	}
}

func generateSamplePositions(start board.BoardState, count int) []board.BoardState {
	positions := []board.BoardState{}
	current := start

	for i := 0; i < count*3 && len(positions) < count; i++ {
		options := current.ValidPlays()
		if len(options) == 0 {
			break
		}

		// Pick a deterministic non-first move to avoid always going down the same edge.
		index := (i*3 + 1) % len(options)

		next := options[index]
		next.SwapTeam()
		current = next

		positions = append(positions, current)
	}

	return positions
}
