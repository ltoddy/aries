pub fn exit(session_id: impl AsRef<str>) {
    let session_id = session_id.as_ref();
    let name = env!("CARGO_BIN_NAME");

    println!("Resume this session with:");
    println!("{name} session resume {session_id}");
    std::process::exit(0);
}
