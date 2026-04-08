pub const NAME: &str = "/exit";

pub fn exit() {
    println!("Goodbye!");
    std::process::exit(0);
}
