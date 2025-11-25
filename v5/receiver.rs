use anyhow::Result;
use futures::TryStreamExt;
use opencv::{core::Vector, highgui, imgcodecs, prelude::*};
use srt_tokio::SrtSocket;

#[tokio::main]
async fn main() -> Result<()> {
    // Connect to the SRT sender at localhost:9999
    println!("Connecting to SRT sender on 127.0.0.1:9999...");
    let mut rx = SrtSocket::builder().call("127.0.0.1:9999", None).await?;
    println!("Connected. Receiving frames...");

    // Create an OpenCV window
    highgui::named_window("SRT Video", highgui::WINDOW_AUTOSIZE)?;

    // Receive loop: read (Instant, Bytes) packets until the sender closes or error occurs
    while let Some((_, bytes)) = rx.try_next().await? {
        // Convert the received Bytes (JPEG data) into a Vec<u8>
        let frame_vec: Vec<u8> = bytes.to_vec();
        // Decode JPEG into an OpenCV Mat (color image)
        let buf = Vector::<u8>::from_iter(frame_vec);
        let frame = imgcodecs::imdecode(&buf, imgcodecs::IMREAD_COLOR)?;
        if frame.empty() {
            eprintln!("Warning: Decoded empty frame");
            break;
        }

        println!("Received frame, displaying...");
        // Display the frame
        highgui::imshow("SRT Video", &frame)?;
        // Wait 1 ms for GUI events (ESC key to break)
        let key = highgui::wait_key(1)?;
        if key == 27 {
            // ESC key
            println!("ESC pressed. Exiting.");
            break;
        }
    }

    println!("Receiver finished.");
    Ok(())
}
