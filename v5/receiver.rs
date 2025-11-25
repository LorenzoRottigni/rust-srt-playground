use anyhow::Result;
use futures::prelude::*;
use opencv::{highgui, imgcodecs, prelude::*};
use srt_tokio::SrtSocket;

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init();

    // Connect to the SRT sender on localhost:9999
    let mut rx = SrtSocket::builder().call("127.0.0.1:9999", None).await?;
    println!("Connected to SRT sender on 127.0.0.1:9999");

    // Create a window to display received frames
    let window = "Received Frame";
    highgui::named_window(window, highgui::WINDOW_AUTOSIZE)?;

    // Loop: receive (timestamp, data) and display frames
    while let Some((timestamp, data)) = rx.try_next().await? {
        println!("Received frame at {:?}", timestamp);

        // Convert the Bytes to a Vec<u8> and decode JPEG into a Mat
        let jpeg_bytes = data.to_vec();
        let buf = Mat::from_slice::<u8>(&jpeg_bytes)?;
        let frame = imgcodecs::imdecode(&buf, imgcodecs::IMREAD_COLOR)?;
        if frame.empty() {
            println!("Empty frame received, stopping.");
            break;
        }

        // Show the frame
        highgui::imshow(window, &frame)?;
        // Wait briefly (e.g. 1 ms) to allow window to update; exit if 'q' is pressed
        if highgui::wait_key(1)? == 'q' as i32 {
            println!("Exit requested by user");
            break;
        }
    }

    Ok(())
}
