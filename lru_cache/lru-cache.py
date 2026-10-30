from __future__ import annotations


class LLNode:
    def __init__(self, key, val):
        self.next: LLNode | None = None
        self.prev: LLNode | None = None
        self.value = val
        self.key = key


class LRUCache:
    def __init__(self, capacity=10):
        self.cache = {}
        self.capacity = capacity
        self.start: LLNode | None = None
        self.end: LLNode | None = None

    def get_middle_node(self, node):
        # example of [A => B => C] and get("B")
        # get B neighbors
        node_a = node.prev
        node_c = node.next
        # Detach B
        node.next = None
        node.prev = None
        # Append B
        node_a.next = node_c
        node_c.prev = node_a
        self.end.next = node
        node.prev = self.end
        # update end to B
        self.end = node

    def get_first_node(self, node):
        # [A => B => C] get("A")
        # get A neighbors
        node_b = node.next
        # Detach A
        node.next = None
        node.prev = None
        # Append A
        node_b.prev = None
        self.end.next = node
        node.prev = self.end
        # update end to A and start to B
        self.end = node
        self.start = node_b

    def get(self, key):
        node = self.cache.get(key, None)
        if node is not None:
            if node is self.end:
                return node.value
            if (node.next and node.prev) is not None:
                self.get_middle_node(node)
            # matches [A] get(A)
            elif node.prev is None:
                self.get_first_node(node)
            return node.value
        else:
            return None

    def put(self, key, val):
        # duplicate node update
        # put("A", 10)
        # [A]
        # put("B", 20)
        # [A, B]
        # put("A", 99)
        # [B, A]
        node = self.cache.get(key, None)
        if node:
            node.value = val
            self.get(key)
            return
        # new node
        else:
            node = LLNode(key, val)
        if self.start is None:
            self.start = node
            self.end = node
        else:
            node.prev = self.end
            node.prev.next = node
            self.end = node
        self.cache[key] = node
        if len(self.cache) > self.capacity:
            evict = self.start
            self.cache.pop(evict.key)
            self.start = evict.next
            if self.start is not None:
                self.start.prev = None
            if self.start is None:
                self.end = None
