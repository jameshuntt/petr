use petr_core::ValueWord;

fn main() {
    let imm = ValueWord::immediate_i31(42);
    let handle = ValueWord::handle(0x1000);
    println!(
        "imm raw={:#x} is_imm={} decoded={:?}",
        imm.raw(),
        imm.is_immediate(),
        imm.as_i31()
    );
    println!(
        "handle raw={:#x} is_imm={}",
        handle.raw(),
        handle.is_immediate()
    );
}
