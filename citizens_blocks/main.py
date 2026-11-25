import argparse
import random
import sys
import time
from collections import deque


W = 30
H = 30
PEOPLE = 5
BOXES = 5
MOVES = {"up": (0, -1), "left": (-1, 0), "right": (1, 0), "down": (0, 1)}
ORDER = ["up", "left", "right", "down"]
COLORS = [39, 207, 82, 214, 141]


def add(p, move):
    d = MOVES[move]
    return (p[0] + d[0], p[1] + d[1])


def inside(p):
    return p[0] >= 0 and p[0] < W and p[1] >= 0 and p[1] < H


def wall(p):
    return p[0] == 0 or p[0] == W - 1 or p[1] == 0 or p[1] == H - 1


def backwards(m):
    return {"up": "down", "down": "up", "left": "right", "right": "left"}[m]


# boring bfs stuff
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
                answer.reverse()
                return answer
            seen.add(n)
            q.append(n)
    return None


class Citizen:
    def __init__(self, number, pos):
        self.id = number
        self.name = chr(ord("A") + number)
        self.pos = pos
        self.box = number
        self.push_way = None

    # This is the citizen interface. world is a snapshot and others has names/ids/positions.
    def step(self, world, others):
        box = tuple(world["blocks"][self.box])
        if wall(box):
            self.push_way = None
            # Finished people wander away from boxes. They can't just sit at a wall because
            # sometimes they are standing in front of somebody else's last push.
            box_spots = {tuple(x) for x in world["blocks"]}
            person_spots = {tuple(x["pos"]) for x in others}
            not_done = [tuple(x) for x in world["blocks"] if not wall(tuple(x))]
            choices = []
            for i, m in enumerate(ORDER):
                n = add(self.pos, m)
                if not inside(n) or n in box_spots or n in person_spots:
                    continue
                if not_done:
                    away = min(abs(n[0] - b[0]) + abs(n[1] - b[1]) for b in not_done)
                else:
                    away = 0
                edge = min(n[0], W - 1 - n[0], n[1], H - 1 - n[1])
                choices.append((-away, edge, i, m))
            if choices:
                choices.sort()
                return choices[0][3]
            return "up"

        blocks = {tuple(x) for x in world["blocks"]}
        people = {tuple(x["pos"]) for x in others}

        def clear(m):
            behind = add(box, backwards(m))
            if not inside(behind) or behind in blocks:
                return False
            p = add(box, m)
            while inside(p):
                if p in blocks and p != box:
                    return False
                if wall(p):
                    return True
                p = add(p, m)
            return False

        def route(m):
            where_i_need_to_stand = add(box, backwards(m))
            blocked = set(blocks) | people
            blocked.discard(self.pos)
            return find_path(self.pos, where_i_need_to_stand, blocked)

        # keep pushing the same way if it still makes sense
        if self.push_way is not None and clear(self.push_way):
            p = route(self.push_way)
            if p is not None:
                if len(p):
                    return p[0]
                return self.push_way

        plans = []
        for rank, m in enumerate(ORDER):
            if not clear(m):
                continue
            p = route(m)
            if p is None:
                continue
            if m == "left":
                pushes = box[0]
            elif m == "right":
                pushes = W - 1 - box[0]
            elif m == "up":
                pushes = box[1]
            else:
                pushes = H - 1 - box[1]
            plans.append((len(p) + pushes, rank, m, p))

        if not plans:
            self.push_way = None
            # just find some empty direction and hope the other boxes get moved soon
            for m in ORDER:
                n = add(self.pos, m)
                if inside(n) and n not in blocks and n not in people:
                    return m
            return "up"

        plans.sort(key=lambda x: (x[0], x[1]))
        best = plans[0]
        self.push_way = best[2]
        if best[3]:
            return best[3][0]
        return self.push_way


def new_game():
    all_places = []
    for y in range(1, H - 1):
        for x in range(1, W - 1):
            all_places.append((x, y))
    picked = random.sample(all_places, PEOPLE + BOXES)
    guys = []
    for i in range(PEOPLE):
        guys.append(Citizen(i, picked[i]))
    boxes = []
    for i in range(BOXES):
        boxes.append([picked[PEOPLE + i][0], picked[PEOPLE + i][1]])
    return {"blocks": boxes, "citizens": guys, "tick": 0}


def do_move(game, guy, move):
    destination = add(guy.pos, move)
    if not inside(destination):
        return
    other_people = {x.pos for x in game["citizens"] if x.id != guy.id}
    if destination in other_people:
        return
    which_box = None
    for i, b in enumerate(game["blocks"]):
        if tuple(b) == destination:
            which_box = i
            break
    if which_box is not None:
        next_box = add(destination, move)
        if not inside(next_box) or next_box in other_people:
            return
        if next_box in {tuple(x) for x in game["blocks"]}:
            return
        game["blocks"][which_box][0] = next_box[0]
        game["blocks"][which_box][1] = next_box[1]
    guy.pos = destination


def game_step(game):
    # They go in ID order. Each one sees moves made earlier in this frame.
    for guy in game["citizens"]:
        world = {
            "width": W,
            "height": H,
            "tick": game["tick"],
            "blocks": [x[:] for x in game["blocks"]],
        }
        others = []
        for x in game["citizens"]:
            if x.id != guy.id:
                others.append({"id": x.id, "name": x.name, "pos": x.pos})
        wanted = guy.step(world, others)
        do_move(game, guy, wanted)
    game["tick"] += 1


def won(game):
    for b in game["blocks"]:
        if not wall(tuple(b)):
            return False
    return True


def paint(s, code, use_it):
    if use_it:
        return code + s + "\x1b[0m"
    return s


def draw(game, color=True):
    people = {x.pos: x for x in game["citizens"]}
    blocks = {tuple(x) for x in game["blocks"]}
    border = paint("██", "\x1b[38;5;240m", color) * (W + 2)
    lines = [border]
    for y in range(H):
        row = [paint("██", "\x1b[38;5;240m", color)]
        for x in range(W):
            p = (x, y)
            if p in people:
                c = COLORS[people[p].id % len(COLORS)]
                row.append(paint("🧙", "\x1b[1m\x1b[38;5;" + str(c) + "m", color))
            elif p in blocks:
                row.append(paint("📦", "\x1b[1m\x1b[38;5;220m", color))
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
    args = parser.parse_args()
    if args.fps <= 0:
        print("fps has to be above zero")
        return
    game = new_game()
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
    result = "WIN" if won(game) else "STOPPED"
    print(result + ": rounds=" + str(game["tick"]))


if __name__ == "__main__":
    main()
