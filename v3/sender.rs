use bytes::Bytes;
use futures::stream;
use futures::{SinkExt, StreamExt};
use srt_tokio::SrtSocket;
use std::io::Error;
use std::time::Instant;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Create an SRT socket and listen on port 1234
    let mut srt_socket = SrtSocket::builder().listen_on(":1234").await?;
    println!("SRT sender listening on :1234...");

    // Stream of dummy "camera frames"
    let mut stream = stream::unfold(0, |count| async move {
        print!("\rSent {count:?} packets");
        sleep(Duration::from_millis(30)).await;

        // Dummy frame bytes
        let packet = Bytes::from(vec![0u8; 8000]);
        Some((Ok((Instant::now(), packet)), count + 1))
    })
    .boxed();

    // Send all packets over SRT
    srt_socket.send_all(&mut stream).await?;
    Ok(())
}
