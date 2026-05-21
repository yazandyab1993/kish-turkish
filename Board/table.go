package board

import "sync"

type storedState struct {
	board BoardState //Stores board state since hash will have collisions
	value float32    //Stores previously evaluated value
	depth int32      //Stores remaining depth from this position
}

type TransposTable struct {
	mu       sync.RWMutex
	internal map[uint64]storedState
}

// Creates a new table and returns a pointer
func NewTable() *TransposTable {
	return &TransposTable{
		internal: make(map[uint64]storedState),
	}
}

// Returns if the board has been previously evaluated, and its value
func (table *TransposTable) Request(board *BoardState, depth int32) (bool, float32) {
	//Hash board state and load entry
	hash := board.hashBoard()
	table.mu.RLock()
	entry, exists := table.internal[hash]
	table.mu.RUnlock()

	if exists { //Check if it exists and if its the exact same board not just a collision
		if entry.board == *board {
			// only reuse an entry when it was searched deeply enough
			if entry.depth+TableDepthAllowedInaccuracy >= depth {
				return true, entry.value
			}
		}
	}
	return false, 0.0 //Otherwise returns that it didn't find anything

}

// Sets the board
func (table *TransposTable) Set(board *BoardState, value float32, depth int32) {
	//Hash board state and write to table
	hash := board.hashBoard()

	table.mu.Lock()
	defer table.mu.Unlock()

	//Replace only if greater depth, ie more computationally expensive
	entry, exists := table.internal[hash]
	if !exists || depth >= entry.depth {
		table.internal[hash] = storedState{*board, value, depth}
	}
}

// Hash combines occupancy, side-to-move and piece metadata.
func (board *BoardState) hashBoard() uint64 {
	const fnvOffset uint64 = 1469598103934665603
	const fnvPrime uint64 = 1099511628211

	h := fnvOffset
	mix := func(v uint64) {
		h ^= v
		h *= fnvPrime
	}

	mix(board.Full)
	mix(board.Team)
	mix(board.King)
	mix(uint64(board.Turn) + 0x9e3779b97f4a7c15)

	return h
}
