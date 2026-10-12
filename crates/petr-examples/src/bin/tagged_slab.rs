use petr_collections::TaggedSlab;

fn main() {
    let mut slab = TaggedSlab::<u64, 3>::new();
    let h = slab.insert(99, 0b011).unwrap();
    println!("handle index={} generation={} tag={}", h.index, h.generation, h.tagged.tag());
    println!("value={:?}", slab.get(h));
    println!("removed={:?}", slab.remove(h));
    println!("occupied={}", slab.occupied());
}
