use bytes::Bytes;
use futures::stream;
use futures::{SinkExt, StreamExt};
use srt_tokio::SrtSocket;
use std::io::Error;
use std::time::Instant;
use tokio::time::{sleep, Duration};

use opencv::{
    core::{Mat, Vector},
    imgcodecs,
    prelude::*,
    videoio::{VideoCapture, CAP_ANY},
};

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Open camera
    let mut cam = VideoCapture::new(0, CAP_ANY).map_err(|e| {
        eprintln!("Failed to open camera: {:?}", e);
        std::io::Error::new(std::io::ErrorKind::Other, "Cannot open camera")
    })?;

    if !cam
        .is_opened()
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "Camera not opened"))?
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Camera not opened",
        ));
    }

    println!("Camera opened successfully.");

    // Create SRT socket and listen
    let mut srt_socket = SrtSocket::builder().listen_on(":1234").await?;
    println!("SRT sender listening on :1234...");

    let mut frame_count = 0usize;

    // Stream frames as a futures stream
    let mut stream = stream::unfold((), |_| {
        async {
            // Capture frame
            let mut frame = Mat::default();
            if !cam.read(&mut frame).unwrap() || frame.empty() {
                eprintln!("Failed to capture frame");
                sleep(Duration::from_millis(30)).await;
                return Some((Err(Error::from(std::io::ErrorKind::Other)), ()));
            }

            // Encode frame as JPEG bytes
            let mut buf = Vector::<u8>::new();
            imgcodecs::imencode(".jpg", &frame, &mut buf, &Vector::new()).unwrap();
            let packet = Bytes::from(buf.to_vec());

            frame_count += 1;
            print!("\rSent frame #{frame_count}");
            sleep(Duration::from_millis(30)).await; // ~30 FPS

            Some((Ok((Instant::now(), packet)), ()))
        }
    })
    .boxed();

    // Send the stream over SRT
    srt_socket.send_all(&mut stream).await?;

    Ok(())
}
