import argparse
import itertools
import random
import re
import sys
import time
from collections import deque

W = 30
H = 30
PEOPLE = 5
BOXES = 5
L_PIECES = 2
MOVES = {"up": (0, -1), "left": (-1, 0), "right": (1, 0), "down": (0, 1)}
ORDER = ["up", "left", "right", "down"]
COLORS = [39, 207, 82, 214, 141, 201, 51]
L_SHAPE = ((0, 0), (1, 0), (2, 0), (3, 0), (0, 1), (0, 2), (0, 3))


def add(p, move):
    d = MOVES[move]
    return p[0] + d[0], p[1] + d[1]


def inside(p):
    return 0 <= p[0] < W and 0 <= p[1] < H


def wall(p):
    return p[0] in (0, W - 1) or p[1] in (0, H - 1)


def backwards(m):
    return {"up": "down", "down": "up", "left": "right", "right": "left"}[m]


def cells(piece):
    x, y = piece["anchor"]
    return {(x + dx, y + dy) for dx, dy in piece["shape"]}


def translate(points, move):
    dx, dy = MOVES[move]
    return {(x + dx, y + dy) for x, y in points}


def rotate_shape(shape, turns):
    points = list(shape)
    for _ in range(turns):
        points = [(-y, x) for x, y in points]
    min_x = min(x for x, _ in points)
    min_y = min(y for _, y in points)
    return tuple(sorted((x - min_x, y - min_y) for x, y in points))


def complete(piece):
    occupied = cells(piece)
    if piece["kind"] == "single":
        return any(wall(p) for p in occupied)
    return any(sum(p[0] == edge for p in occupied) == 4 for edge in (0, W - 1)) or any(
        sum(p[1] == edge for p in occupied) == 4 for edge in (0, H - 1)
    )


def can_push_to_wall(piece, move, blockers):
    """Whether this straight push reaches a completed position without hitting another piece."""
    projected = {**piece, "anchor": piece["anchor"]}
    while not complete(projected):
        next_cells = translate(cells(projected), move)
        if not all(inside(p) for p in next_cells) or next_cells & blockers:
            return False
        projected["anchor"] = add(projected["anchor"], move)
    return True


def push_positions(piece, move, occupied):
    return sorted({add(cell, backwards(move)) for cell in cells(piece)
                   if inside(add(cell, backwards(move))) and add(cell, backwards(move)) not in occupied})


def coordinated_contacts(piece, move, occupied, count):
    positions = push_positions(piece, move, occupied)
    groups = (group for group in itertools.combinations(positions, count)
              if all(abs(a[0] - b[0]) <= 2 and abs(a[1] - b[1]) <= 2
                     for a, b in itertools.combinations(group, 2)))
    return min(groups, key=lambda group: (sum(abs(a[0] - b[0]) + abs(a[1] - b[1])
                                               for a, b in itertools.combinations(group, 2)), group), default=None)


def team_l_contacts(piece, move, occupied, leader):
    return coordinated_contacts(piece, move, occupied, 2)


def team_l_plan(pieces, leader):
    piece_id = BOXES + leader // 2
    piece = next((p for p in pieces if p["id"] == piece_id and p["kind"] == "L" and not p["done"]), None)
    if piece is None:
        return None
    occupied = {p for block in pieces for p in cells(block)}
    blockers = occupied - cells(piece)
    for move in ORDER:
        if can_push_to_wall(piece, move, blockers) and team_l_contacts(piece, move, occupied, leader):
            return piece, move
    return None


def reachable_team_contacts(piece, move, occupied, citizen_positions, citizen_ids, reserved, leader):
    slots = push_positions(piece, move, occupied)
    preferred = team_l_contacts(piece, move, occupied, leader)
    pairs = list(itertools.combinations(slots, 2))
    pairs.sort(key=lambda pair: (pair != preferred, pair))
    first, second = citizen_ids
    outsiders = {pos for cid, pos in citizen_positions.items() if cid not in citizen_ids}
    for pair in pairs:
        if any(abs(pair[0][axis] - pair[1][axis]) > 2 for axis in (0, 1)):
            continue
        for first_goal, second_goal in (pair, pair[::-1]):
            if first_goal in reserved or second_goal in reserved or first_goal in outsiders or second_goal in outsiders:
                continue
            first_blockers = occupied | {pos for cid, pos in citizen_positions.items() if cid != first} | reserved
            second_blockers = occupied | {pos for cid, pos in citizen_positions.items() if cid != second} | reserved
            if (find_path(citizen_positions[first], first_goal, first_blockers) is not None
                    and find_path(citizen_positions[second], second_goal, second_blockers) is not None):
                return first_goal, second_goal
    return None


def l_assignments(citizen_positions, pieces):
    """Recomputed independently from one snapshot; reservations are not push permission."""
    occupied = {p for block in pieces for p in cells(block)}
    assignments = {}
    efforts = []
    for leader in (0, 2):
        piece_id = BOXES + leader // 2
        piece = next((p for p in pieces if p["id"] == piece_id and p["kind"] == "L" and not p["done"]), None)
        citizen_ids = (leader, leader + 1)
        if piece is None or not set(citizen_ids) <= citizen_positions.keys():
            continue
        blockers = occupied - cells(piece)
        for move in ORDER:
            if not can_push_to_wall(piece, move, blockers):
                continue
            contacts = reachable_team_contacts(piece, move, occupied, citizen_positions, citizen_ids, set(), leader)
            if contacts is None:
                continue
            efforts.append((piece, move, leader, contacts))
            for cid, goal in zip(citizen_ids, contacts):
                assignments[cid] = (piece, move, leader, goal)
            break

    reserved = {assignment[3] for assignment in assignments.values()}
    for cid, position in sorted(citizen_positions.items()):
        if cid in assignments:
            continue
        candidates = []
        blockers = occupied | {p for other, p in citizen_positions.items() if other != cid}
        for piece, move, leader, contacts in efforts:
            for goal in push_positions(piece, move, occupied):
                if goal in reserved or goal in blockers:
                    continue
                if not all(abs(goal[0] - p[0]) <= 2 and abs(goal[1] - p[1]) <= 2 for p in contacts):
                    continue
                path = find_path(position, goal, blockers | (reserved - {position}))
                if path is not None:
                    candidates.append((len(path), piece["id"], goal, move, leader, piece))
        if candidates:
            _, _, goal, move, leader, piece = min(candidates, key=lambda c: c[:3])
            assignments[cid] = (piece, move, leader, goal)
            reserved.add(goal)
    return assignments


def single_assignment(citizen_id, citizen_positions, pieces, assignments=None):
    active = l_assignments(citizen_positions, pieces) if assignments is None else assignments
    available_people = [cid for cid in sorted(citizen_positions) if cid not in active]
    singles = [p for p in pieces if p["kind"] == "single" and not p["done"]]
    assignments = {}
    for cid in available_people:
        if not singles:
            break
        position = citizen_positions[cid]
        piece = min(singles, key=lambda p: (abs(position[0] - p["anchor"][0]) + abs(position[1] - p["anchor"][1]), p["id"]))
        assignments[cid] = piece
        singles.remove(piece)
    return assignments.get(citizen_id)


def find_path(start, end, things):
    if start == end:
        return []
    if end in things:
        return None
    q = deque([start])
    seen = {start}
    old = {}
    while q:
        p = q.popleft()
        for m in ORDER:
            n = add(p, m)
            if not inside(n) or n in seen or n in things:
                continue
            old[n] = (p, m)
            if n == end:
                answer = []
                here = n
                while here != start:
                    before, how = old[here]
                    answer.append(how)
                    here = before
                return answer[::-1]
            seen.add(n)
            q.append(n)
    return None


class Citizen:
    def __init__(self, number, pos):
        self.id = number
        self.name = chr(ord("A") + number)
        self.pos = pos
        self.push_way = None
        self.target = None
        self.plan_key = None
        self.accepted_plan = None
        self.ready_plan = None
        self.negotiations = {}
        self.generation = 0
        self.last_attempt = {}
        self.steps = 0
        self.last_debug = ""

    def step(self, world, others, heard):
        self.last_debug = ""
        pieces = world["blocks"]
        all_cells = {p for piece in pieces for p in cells(piece)}
        people = {tuple(x["pos"]) for x in others}
        positions = {x["id"]: tuple(x["pos"]) for x in others}
        positions[self.id] = self.pos
        assignments = l_assignments(positions, pieces)
        reserved = {assignment[3] for cid, assignment in assignments.items() if cid != self.id}
        plan = assignments.get(self.id)
        key = None if plan is None else (plan[0]["id"], tuple(plan[0]["anchor"]), *plan[1:])
        if key != self.plan_key:
            self.plan_key = key
            self.accepted_plan = self.ready_plan = None
            self.negotiations.clear()
        self.steps += 1
        now = world.get("tick", self.steps)
        if plan:
            target, move, leader, goal = plan
            self.target = target["id"]
            peers = ([cid for cid, assignment in sorted(assignments.items())
                      if cid != leader and assignment[2] == leader] if self.id == leader else [leader])
            self.negotiations = {peer: state for peer, state in self.negotiations.items() if peer in peers}
            speeches = []
            may_push = False
            for peer in peers:
                leader_goal = assignments[leader][3]
                follower = peer if self.id == leader else self.id
                follower_goal = assignments[follower][3]
                slots = f"{leader_goal[0]},{leader_goal[1]}:{follower_goal[0]},{follower_goal[1]}"
                prefix = f"piece {target['id']} {move}"
                pattern = re.compile(rf"Citizen {self.id}, (PROPOSE|ACCEPT|CONFIRM|READY) "
                                     rf"{prefix} attempt (\d+) slots {slots}")
                messages = []
                for msg in heard:
                    if msg["speaker_id"] != peer:
                        continue
                    for line in msg["text"].splitlines():
                        match = pattern.fullmatch(line)
                        if match:
                            messages.append((match[1], int(match[2])))
                peer_key = (key, peer, assignments[peer][3])
                state = self.negotiations.get(peer)
                if state and (state["key"] != peer_key or now - state["started"] >= 24):
                    self.negotiations.pop(peer)
                    self.accepted_plan = self.ready_plan = None
                    state = None
                action = None
                if self.id == leader:
                    pending = state is not None
                    if state is None:
                        self.generation += 1
                        state = {"key": peer_key, "attempt": self.generation, "started": now,
                                 "accepted": False, "ready": False}
                        self.negotiations[peer] = state
                    if pending and state["accepted"] and ("READY", state["attempt"]) in messages:
                        state["ready"] = True
                    if pending and ("ACCEPT", state["attempt"]) in messages:
                        state["accepted"] = True
                    action = "CONFIRM" if state["accepted"] else "PROPOSE"
                    # Optional recruits never gate the primary pair.
                    if peer == leader + 1:
                        may_push = (state["ready"] and positions[peer] == follower_goal)
                else:
                    acknowledged = state is not None
                    for verb, attempt in messages:
                        if verb == "PROPOSE" and (attempt > self.last_attempt.get(peer, 0)
                                                  or state and attempt == state["attempt"]):
                            if state is None or state["attempt"] != attempt:
                                self.ready_plan = None
                                acknowledged = False
                                state = {"key": peer_key, "attempt": attempt, "started": now, "ready": False}
                                self.negotiations[peer] = state
                            self.last_attempt[peer] = attempt
                            self.accepted_plan = (peer_key, attempt)
                    if state:
                        action = "ACCEPT"
                        if acknowledged and ("CONFIRM", state["attempt"]) in messages and self.pos == goal:
                            may_push = state["ready"] and positions[leader] == leader_goal
                            state["ready"] = True
                            self.ready_plan = (peer_key, state["attempt"])
                            action = "READY"
                if action:
                    speeches.append(f"Citizen {peer}, {action} {prefix} attempt {state['attempt']} slots {slots}")

            path = find_path(self.pos, goal, all_cells | people | reserved)
            self.last_debug = f"L#{target['id']} {move}; goal={goal}; path={len(path) if path is not None else 'unreachable'}; push_ready={may_push}"
            return {"move": path[0] if path else move if self.pos == goal and may_push else None,
                    "speak": "\n".join(speeches) or None}

        self.target = None
        # Use the same reservations for singles, so helpers cannot be double-booked.
        piece = single_assignment(self.id, positions, pieces, assignments)
        if piece:
            box = piece["anchor"]
            for move in ([self.push_way] if self.push_way else []) + [m for m in ORDER if m != self.push_way]:
                if move is None:
                    continue
                behind = add(box, backwards(move))
                goal = add(box, move)
                if not inside(goal) or goal in all_cells | reserved or behind in all_cells | reserved:
                    continue
                path = find_path(self.pos, behind, all_cells | people | reserved)
                if path is not None:
                    self.push_way = move
                    self.last_debug = f"single#{piece['id']} {move}; path={len(path)}"
                    return {"move": path[0] if path else move, "speak": None}

        # Keep unassigned citizens out of reserved L contacts as well as occupied cells.
        for move in ORDER:
            n = add(self.pos, move)
            if inside(n) and n not in all_cells | people | reserved:
                self.last_debug += ("; " if self.last_debug else "") + f"fallback wander {move}"
                return {"move": move, "speak": None}
        self.last_debug += ("; " if self.last_debug else "") + "no legal move"
        return {"move": None, "speak": None}


def new_game():
    occupied = set()
    pieces = []
    for i in range(BOXES):
        while True:
            p = (random.randrange(1, W - 1), random.randrange(1, H - 1))
            if p not in occupied:
                occupied.add(p)
                pieces.append({"id": i, "kind": "single", "anchor": p, "shape": ((0, 0),), "done": False})
                break
    for i in range(L_PIECES):
        shape = rotate_shape(L_SHAPE, random.randrange(4))
        for _ in range(10000):
            anchor = (random.randrange(1, W - 5), random.randrange(1, H - 5))
            piece = {"id": BOXES + i, "kind": "L", "anchor": anchor, "shape": shape, "done": False}
            if all(inside(p) and p not in occupied for p in cells(piece)):
                occupied.update(cells(piece))
                pieces.append(piece)
                break
        else:
            raise RuntimeError("could not place L piece without overlap")
    free = [(x, y) for y in range(1, H - 1) for x in range(1, W - 1) if (x, y) not in occupied]
    guys = [Citizen(i, p) for i, p in enumerate(random.sample(free, PEOPLE))]
    return {
        "blocks": pieces,
        "citizens": guys,
        "tick": 0,
        "speech": {g.id: [] for g in guys},
        "world": {"width": W, "height": H, "tick": 0, "blocks": []},
        "debug": False,
    }


def game_step(game):
    # All decisions use the same tick-start snapshot; physical changes are committed together.
    snapshot_pieces = [{**p, "shape": tuple(p["shape"])} for p in game["blocks"]]
    world = game.setdefault("world", {})
    world.update({"width": W, "height": H, "tick": game["tick"], "blocks": snapshot_pieces})
    intents = {}
    decisions = {}
    speech = []
    for guy in game["citizens"]:
        others = [{"id": x.id, "name": x.name, "pos": x.pos} for x in game["citizens"] if x.id != guy.id]
        heard = game["speech"].get(guy.id, [])
        decision = guy.step(world, others, heard)
        move = decision.get("move")
        if move not in MOVES and move is not None:
            move = None
        intents[guy.id] = move
        decisions[guy.id] = (guy.pos, move, decision.get("speak"), list(heard), getattr(guy, "last_debug", ""))
        text = decision.get("speak")
        if text:
            speech.append((guy.id, guy.pos, text))

    starts = {g.id: g.pos for g in game["citizens"]}
    pieces_at = {p: piece for piece in snapshot_pieces for p in cells(piece)}
    pushers = {}
    walks = {}
    for guy in game["citizens"]:
        move = intents[guy.id]
        if move is None:
            continue
        destination = add(starts[guy.id], move)
        if destination in pieces_at:
            piece = pieces_at[destination]
            pushers.setdefault(piece["id"], {}).setdefault(move, []).append(guy.id)
        else:
            walks[guy.id] = (destination, move)

    moving = {}
    push_status = {}
    for piece in snapshot_pieces:
        proposals = pushers.get(piece["id"], {})
        if not proposals:
            push_status[piece["id"]] = "no push attempts"
            continue
        if len(proposals) != 1:
            push_status[piece["id"]] = f"conflicting directions {proposals}"
            continue
        move, ids = next(iter(proposals.items()))
        required = 2 if piece["kind"] == "L" else 1
        if len(ids) < required:
            push_status[piece["id"]] = f"{len(ids)}/{required} pushers for {move}"
            continue
        old_cells = cells(piece)
        new_cells = translate(old_cells, move)
        if not all(inside(p) for p in new_cells):
            push_status[piece["id"]] = f"{move} blocked by board edge"
            continue
        other_piece_cells = {p for other in snapshot_pieces if other["id"] != piece["id"] for p in cells(other)}
        if new_cells & other_piece_cells:
            blockers = sorted(new_cells & other_piece_cells)
            push_status[piece["id"]] = f"{move} blocked by piece(s) at {blockers}"
            continue
        if new_cells & {position for cid, position in starts.items() if cid not in ids}:
            push_status[piece["id"]] = f"{move} blocked by citizen"
            continue
        moving[piece["id"]] = (move, set(ids), old_cells, new_cells)
        push_status[piece["id"]] = f"accepted {move} with citizens {sorted(ids)}"

    # Keep valid piece pushes independent from unrelated walking conflicts.
    accepted = dict(moving)
    for piece_id, (_, _, _, new_cells) in moving.items():
        if any(new_cells & other[3] for other_id, other in moving.items() if other_id != piece_id):
            accepted.pop(piece_id, None)
            push_status[piece_id] = "rejected: overlaps another moving piece"

    final_piece_cells = {p for piece in snapshot_pieces for p in
                         (accepted[piece["id"]][3] if piece["id"] in accepted else cells(piece))}
    proposed_people = dict(starts)
    push_destinations = set()
    for piece_id, (_, ids, _, _) in accepted.items():
        destinations = {cid: add(starts[cid], intents[cid]) for cid in ids}
        if (len(set(destinations.values())) != len(destinations)
                or any(p in final_piece_cells - accepted[piece_id][3] or p in starts.values() for p in destinations.values())):
            accepted.pop(piece_id)
            push_status[piece_id] = "rejected: pusher destination blocked"
            continue
        for cid, destination in destinations.items():
            proposed_people[cid] = destination
            push_destinations.add(destination)

    occupied_starts = set(starts.values())
    for cid, (destination, _) in sorted(walks.items()):
        if (not inside(destination) or destination in final_piece_cells or destination in occupied_starts - {starts[cid]}
                or destination in push_destinations or destination in proposed_people.values()):
            if game.get("debug"):
                print(f"[tick {game['tick']}] citizen {cid} walk to {destination} blocked; accepted piece pushes remain intact", file=sys.stderr)
            continue
        proposed_people[cid] = destination
    for piece in game["blocks"]:
        if piece["id"] in accepted:
            move = accepted[piece["id"]][0]
            piece["anchor"] = add(piece["anchor"], move)
    for guy in game["citizens"]:
        guy.pos = proposed_people[guy.id]
    for piece in game["blocks"]:
        piece["done"] = complete(piece)

    inboxes = {g.id: [] for g in game["citizens"]}
    for speaker, position, text in speech:
        for guy in game["citizens"]:
            recipient_position = starts[guy.id]
            if guy.id != speaker and abs(position[0] - recipient_position[0]) <= 2 and abs(position[1] - recipient_position[1]) <= 2:
                inboxes[guy.id].append({"speaker_id": speaker, "text": text})
    if game.get("debug"):
        print(f"[tick {game['tick']}]", file=sys.stderr)
        for guy in game["citizens"]:
            start, move, text, heard, note = decisions[guy.id]
            result = "moved" if guy.pos != start else "stayed"
            print(f"  citizen {guy.id} {start} -> {guy.pos}: wanted={move}; {result}; {note}; heard={heard}; said={text!r}", file=sys.stderr)
        for piece in snapshot_pieces:
            if piece["kind"] == "L":
                print(f"  L piece {piece['id']} at {piece['anchor']}: {push_status[piece['id']]}", file=sys.stderr)
    game["speech"] = inboxes
    game["tick"] += 1


def won(game):
    return all(complete(piece) for piece in game["blocks"])


def paint(s, code, use_it):
    return code + s + "\x1b[0m" if use_it else s


def draw(game, color=True):
    people = {x.pos: x for x in game["citizens"]}
    piece_cells = {p: piece for piece in game["blocks"] for p in cells(piece)}
    border = paint("██", "\x1b[38;5;240m", color) * (W + 2)
    lines = [border]
    for y in range(H):
        row = [paint("██", "\x1b[38;5;240m", color)]
        for x in range(W):
            p = (x, y)
            if p in people:
                c = COLORS[people[p].id % len(COLORS)]
                row.append(paint("🧙", "\x1b[1m\x1b[38;5;" + str(c) + "m", color))
            elif p in piece_cells:
                piece = piece_cells[p]
                c = COLORS[piece["id"] % len(COLORS)]
                row.append(paint("📦", "\x1b[1m\x1b[38;5;" + str(c) + "m", color))
            else:
                row.append(paint("· ", "\x1b[2m", color))
        row.append(paint("██", "\x1b[38;5;240m", color))
        lines.append("".join(row))
    lines.append(border)
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--fps", type=float, default=2)
    parser.add_argument("--max-steps", type=int, default=2000)
    parser.add_argument("--no-color", action="store_true")
    parser.add_argument("--debug", action="store_true", help="log citizen decisions, speech, and rejected L pushes to stderr")
    args = parser.parse_args()
    if args.fps <= 0:
        print("fps has to be above zero")
        return
    game = new_game()
    game["debug"] = args.debug
    use_color = not args.no_color and sys.stdout.isatty()
    print("\x1b[2J\x1b[?25l", end="", flush=True)
    try:
        while True:
            started = time.monotonic()
            print("\x1b[H" + draw(game, use_color), end="", flush=True)
            if won(game) or game["tick"] >= args.max_steps:
                break
            game_step(game)
            left = 1 / args.fps - (time.monotonic() - started)
            if left > 0:
                time.sleep(left)
    except KeyboardInterrupt:
        pass
    finally:
        print("\x1b[?25h")
    print(("WIN" if won(game) else "STOPPED") + ": rounds=" + str(game["tick"]))


if __name__ == "__main__":
    main()
