use petr_collections::KeyedTaggedVec;
use petr_core::Key;

fn main() {
    let key = Key::<3>::from_seed(0xABCD_1234);
    let mut vec = KeyedTaggedVec::<String, 3>::with_key(key);
    let idx = vec.push("secret-object".to_string(), 0b101).unwrap();
    let handle = vec.handle(idx).unwrap();
    println!("stored masked word = {:#x}, low_tag={:#b}", handle.stored_word(), handle.tag());
    let found = vec.get_by_handle(handle).unwrap().unwrap();
    println!("revealed value = {found}");
}
