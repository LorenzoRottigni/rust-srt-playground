use futures::StreamExt;
use srt_tokio::SrtSocket;
use std::io::Error;

#[tokio::main]
async fn main() -> Result<(), Error> {
    println!("Connecting to SRT sender...");

    // Connect to sender at 127.0.0.1:1234
    let mut srt_socket = SrtSocket::builder().call("127.0.0.1:1234", None).await?;
    println!("Connected! Receiving packets...");

    let mut total_bytes = 0usize;
    let mut packet_count = 0usize;

    while let Some(result) = srt_socket.next().await {
        match result {
            Ok((_instant, bytes)) => {
                total_bytes += bytes.len();
                packet_count += 1;
                println!(
                    "Packet #{} received: {} bytes (total {} bytes)",
                    packet_count,
                    bytes.len(),
                    total_bytes
                );
            }
            Err(e) => eprintln!("Error receiving packet: {e}"),
        }
    }

    println!("SRT stream closed");
    Ok(())
}
