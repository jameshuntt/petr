use petr_core::SelectorWord;

fn main() {
    let encryptor_trait = 0x1;
    let aes_impl = 0xA;
    let write_method = 0x42;
    let selector = SelectorWord::new(encryptor_trait, aes_impl, write_method);
    println!("raw={:#06x}", selector.raw());
    println!("trait={} impl={} function={:#04x}", selector.trait_id(), selector.impl_id(), selector.function_id());
}
