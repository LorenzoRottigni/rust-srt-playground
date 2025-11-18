use anyhow::Result;
use bytes::{BufMut, Bytes, BytesMut};
use futures::SinkExt;
use opencv::{core::Vector, imgcodecs, prelude::*, videoio};
use srt_tokio::SrtSocket;
use std::time::{Duration, Instant};
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<()> {
    println!("Opening camera...");
    let mut cam = videoio::VideoCapture::new(0, videoio::CAP_ANY)?;
    if !videoio::VideoCapture::is_opened(&cam)? {
        panic!("Cannot open camera");
    }
    println!("Camera opened successfully");

    println!("Connecting to SRT receiver...");
    let mut srt = SrtSocket::builder().call("127.0.0.1:4200", None).await?;
    println!("Connected to SRT receiver");

    let mut frame_count = 0;
    loop {
        let mut frame = Mat::default();
        cam.read(&mut frame)?;
        if frame.empty() {
            continue;
        }

        frame_count += 1;
        println!("Captured frame #{}", frame_count);

        // Encode frame to JPEG with compression
        let mut buf = Vector::new();
        let mut params = Vector::<i32>::new();
        params.push(imgcodecs::IMWRITE_JPEG_QUALITY);
        params.push(50); // reduce quality for smaller frames
        imgcodecs::imencode(".jpg", &frame, &mut buf, &params)?;
        println!("Frame encoded, {} bytes", buf.len());

        // Prepend frame length (u32, network byte order)
        let mut frame_packet = BytesMut::with_capacity(4 + buf.len());
        frame_packet.put_u32(buf.len() as u32);
        frame_packet.put_slice(buf.as_slice());

        // Split into SRT packets (~1200 bytes each)
        let packet_size = 1200;
        let mut chunk_count = 0;
        for chunk in frame_packet.as_ref().chunks(packet_size) {
            let now = Instant::now();
            let bytes_chunk = Bytes::copy_from_slice(chunk);
            srt.send((now, bytes_chunk)).await?;
            chunk_count += 1;
        }
        println!("Sent frame #{} in {} packets", frame_count, chunk_count);

        // Yield to avoid blocking SRT
        tokio::task::yield_now().await;
        sleep(Duration::from_millis(33)).await; // ~30 FPS
    }
}
