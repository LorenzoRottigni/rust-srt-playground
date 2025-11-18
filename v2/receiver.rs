use futures::StreamExt;
use srt_tokio::SrtSocket;
use std::io::Error;

#[tokio::main]
async fn main() -> Result<(), Error> {
    println!("Connecting to SRT sender...");

    let mut srt_socket = SrtSocket::builder().call("127.0.0.1:1234", None).await?;

    println!("Connected! Receiving packets...");

    let mut total_bytes: usize = 0;
    let mut packet_count: usize = 0;

    // Use `.next()` to continuously await packets
    while let Some(result) = srt_socket.next().await {
        match result {
            Ok((_instant, bytes)) => {
                let len = bytes.len();
                total_bytes += len;
                packet_count += 1;
                println!(
                    "Packet #{} received: {} bytes (total {} bytes)",
                    packet_count, len, total_bytes
                );
            }
            Err(e) => {
                eprintln!("Error receiving packet: {e}");
            }
        }
    }

    println!("SRT stream closed");
    Ok(())
}
