use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8000").await?;

    println!("Sphinx is listening on http://127.0.0.1:8000");

    axum::serve(listener, sphinx::app()).await?;

    Ok(())
}
