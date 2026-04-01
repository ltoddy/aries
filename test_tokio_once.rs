use tokio::sync::OnceCell;

#[tokio::main]
async fn main() {
    let cell = OnceCell::new();
    let value = cell.get_or_init(|| async { "hello".to_string() }).await;
    println!("{}", value);
}
