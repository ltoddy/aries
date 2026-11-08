pub fn exit(session_id: &str) {
    let name = env!("CARGO_BIN_NAME");

    println!("Resume this session with:");
    println!("{name} session resume {session_id}");
    std::process::exit(0);
}
