use std::collections::HashMap;

struct Cache {
    capacity: usize,
    counter: usize,
    entries: HashMap<String, (i32, usize)>,
}

impl Cache {
    fn put(&mut self, key: String, value: i32) {
        self.counter += 1;
        self.entries.insert(key, (value, self.counter));
        if self.entries.len() > self.capacity {
            println!("Capacity exceeded");
            let oldest = self.entries.iter().min_by_key(|(_, (_, counter))| *counter);
            if let Some((key, _)) = oldest {
                let copy = key.clone();
                self.entries.remove(&copy);
            }
        }
    }

    fn get(&mut self, key: String) -> Option<i32> {
        let result = self.entries.get_mut(&key);
        // Hit
        if let Some((value, counter)) = result {
            self.counter += 1;
            *counter = self.counter;
            return Some(*value);
        }
        // Miss
        None
    }
}
fn main() {
    let mut cache: Cache = Cache {
        capacity: 2,
        counter: 0,
        entries: HashMap::new(),
    };
    cache.put("A".to_string(), 10);
    cache.put("B".to_string(), 20);
    println!("{:?}", cache.get("A".to_string()));
    // cache.put("A".to_string(), 99);
    cache.put("C".to_string(), 30);
    println!("{:?}", cache.entries);
}
