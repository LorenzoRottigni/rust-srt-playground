use anyhow::Result;
use bytes::Bytes;
use futures::SinkExt;
use opencv::core::Vector;
use opencv::imgcodecs::{imencode, ImwriteFlags};
use opencv::prelude::*;
use opencv::videoio::{VideoCapture, CAP_ANY};
use srt_tokio::SrtSocket;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init();

    // Open default camera (index 0)
    let mut cap = VideoCapture::new(0, CAP_ANY)?;
    if !cap.is_opened()? {
        panic!("Unable to open default camera");
    }

    // Create SRT sender socket listening on port 9999
    let mut tx = SrtSocket::builder().listen_on(9999).await?;
    println!("SRT sender listening on port 9999, streaming camera...");

    // Loop: capture frames, encode to JPEG, and send
    loop {
        let mut frame = Mat::default();
        cap.read(&mut frame)?;
        if frame.empty() {
            // No frame captured (e.g., camera disconnected)
            break;
        }

        // Encode frame to JPEG bytes
        let mut buf = Vector::<u8>::new();
        let params = Vector::new(); // default JPEG params
        imencode(".JPG", &frame, &mut buf, &params)?;
        let jpeg_bytes = buf.to_vec();

        // Send (timestamp, data) over SRT
        let now = std::time::Instant::now();

        println!("Sending frame...");
        // The SrtSocket Sink expects (Instant, Bytes) tuples
        tx.send((now, Bytes::from(jpeg_bytes))).await?;

        // Throttle loop to camera FPS (~30 ms per frame for ~30 FPS)
        sleep(Duration::from_millis(30)).await;
    }

    // Close the SRT socket after streaming
    tx.close_and_finish().await?;
    println!("Sender finished streaming.");
    Ok(())
}
