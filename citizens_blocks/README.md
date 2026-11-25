# Multi-agent planning with centralized conflict resolution

## Lab Overview

This lab is a 30x30 game board. There are boxes and people. A person is called a Citizen. The goal is for each citizen to find a path to push a box to any of the board's edges. The game is represented by a snapshot of the world for each turn. After enough turns if all the boxes are against a wall the game is over.

The original game is a reactive, greedy multi-agent heuristic using shortest-path walking. It is not a complete puzzle solver and may not finish.

The algorithm uses greedy box pushing with breadth-first search for walking. It finds a shortest walking route in the current snapshot. It chooses a plan using a scoring system, lowest score wins which is the choice of the direction to walk/push. It keeps the existing pushing direction when feasible rather than calculating a new score to reduce direction switching.

Each citizen only knows its next move, walk or push. It observes the updated world each turn and repeats the process. If no box exists to push it enters a "wandering" mode where it moves away from unfinished boxes to reduce interference to other citizens.

## Implement a feature, L shaped pieces and multi-agent coordination

The feature in `.tickets/new-l-shape-block-with-speech.md` asks for two things.

1. A new block shape which requires 2 or more Citizens to push
2. Citizens can speak to others using the English language

The implementation preserves BFS but introduces multi-cell geometry, deterministic team and contact assignments, a speech-driven readiness state machine and simultaneous collision resolution.

This algorithm chooses the first feasible direction rather than the lowest walk/push score as in the original. Citizens not assigned to an L shape are assigned distinct unfinished single-cell boxes using a Manhattan distance greedy algorithm to assign nearby single-cell boxes in citizen ID order.

Each citizen now runs a small finite-state protocol with a team member. Messages identify the recipient, piece, direction, attempt number, and pushing positions. Citizens can walk toward their positions while negotiating.

An important distinction: speech does not discover the teams or negotiate the destination.
Every citizen independently calculates those assignments from the same whole-world
snapshot. Speech confirms participation and readiness for the already-calculated plan.

### A table comparing the different decision strategies in each version

Note that the feature's single-cell strategy is **less forward-looking** than the original.

┌─────────────────┬─────────────────────────────┬─────────────────────────────────────────┐
│ Decision        │ Original                    │ Feature                                 │
├─────────────────┼─────────────────────────────┼─────────────────────────────────────────┤
│ Box ownership   │ Permanent citizen-to-box    │ Reassigned among available citizens     │
│                 │ mapping                     │                                         │
├─────────────────┼─────────────────────────────┼─────────────────────────────────────────┤
│ Direction       │ Minimize walking distance   │ Keep previous direction, otherwise      │
│ selection       │ plus pushes                 │ first feasible direction                │
├─────────────────┼─────────────────────────────┼─────────────────────────────────────────┤
│ Obstacle        │ Check entire corridor to    │ Check only the next box destination     │
│ lookahead       │ wall                        │                                         │
├─────────────────┼─────────────────────────────┼─────────────────────────────────────────┤
│ Walking         │ BFS                         │ BFS                                     │
├─────────────────┼─────────────────────────────┼─────────────────────────────────────────┤
│ Turn model      │ Sequential                  │ Simultaneous                            │
└─────────────────┴─────────────────────────────┴─────────────────────────────────────────┘

## Running

The original is run with `python main.py --fps 15` or omit the fps param to run slower. The feature is run with `python agent-coordination.py --fps 15`. Each version is not guaranteed to have a solvable board.
