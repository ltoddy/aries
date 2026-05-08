pub fn exit(session_id: &str) {
    let name = env!("CARGO_BIN_NAME");

    println!("Goodbye!");
    println!("Resume this session with: {name} session resume {session_id}");
    std::process::exit(0);
}
