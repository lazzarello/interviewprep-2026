# L shapes and citizen coordination

## Goal

We want to add a new feature — big pieces and citizen coordination.
In addition to the current 5 single-cell pieces, we're going to introduce 2 large pieces. The large pieces are shaped like an L with 7 cells — 3 on each leg, one in the middle:

```
x x x x
x
x
x
```

* The large pieces are placed randomly with random rotation at game start.
* At least 2 citizens are needed to push a large piece; they all need to push it in the same direction. Otherwise it won't move.
* At the end, each L shape needs to be one side completely to the wall. Touching the wall with the end of a leg doesn’t count.
* A good idea to use different colors for different shapes for visibility.
* Citizens can emit speech in their step result and every citizen who is adjacent to them hears their speech as input to their step function. The speech they hear is tagged by the speaker citizen id. Adjacency is defined as being within 2 cells horizontally and vertically.
* The algorithm doesn’t have to be perfect, but is expected to mostly work.

### Speech rules

* Speech must be in English language.
* Speech can be used by agents to coordinate on how they move large pieces.
* Citizens can address each other by ID/number if a targeted speech is needed.
* Citizens can refer to pieces by their IDs.

## Acceptance

* Every citizen is an independent decision maker.
* Each citizen takes in a view of the whole world.
* Each citizen can decide left/right/top/bottom
* The world enforces some physical constraints.
