use anyhow::Result;
use bytes::{Buf, Bytes};
use futures::StreamExt;
use opencv::{core::Vector, highgui, imgcodecs, prelude::*};
use srt_tokio::SrtSocket;
use std::collections::VecDeque;

#[tokio::main]
async fn main() -> Result<()> {
    println!("Listening on SRT port 4200...");
    let mut srt = SrtSocket::builder().listen_on("0.0.0.0:4200").await?;
    println!("SRT listener ready");

    let mut frame_buffer: Vec<u8> = Vec::new();
    let mut frames_queue: VecDeque<Vec<u8>> = VecDeque::new();
    let mut expected_frame_size: Option<usize> = None;

    highgui::named_window("SRT Receiver", highgui::WINDOW_AUTOSIZE)?;
    let mut frame_count = 0;

    while let Some(Ok((_timestamp, bytes_chunk))) = srt.next().await {
        // Append received chunk
        frame_buffer.extend_from_slice(&bytes_chunk);
        println!(
            "Received chunk, {} bytes, buffer size {}",
            bytes_chunk.len(),
            frame_buffer.len()
        );

        loop {
            if expected_frame_size.is_none() {
                if frame_buffer.len() >= 4 {
                    // Read first 4 bytes as u32 frame length
                    let mut length_bytes = &frame_buffer[..4];
                    let len = length_bytes.get_u32() as usize;
                    expected_frame_size = Some(len);
                    frame_buffer.drain(0..4);
                } else {
                    break; // wait for more data
                }
            }

            if let Some(frame_len) = expected_frame_size {
                if frame_buffer.len() >= frame_len {
                    let frame_bytes = frame_buffer.drain(0..frame_len).collect::<Vec<u8>>();
                    expected_frame_size = None;
                    frame_count += 1;
                    println!(
                        "Frame #{} complete, {} bytes",
                        frame_count,
                        frame_bytes.len()
                    );
                    frames_queue.push_back(frame_bytes);
                } else {
                    break; // wait for full frame
                }
            }
        }

        // Display available frames
        while let Some(frame_bytes) = frames_queue.pop_front() {
            let buf = Vector::from_slice(&frame_bytes);
            let frame = imgcodecs::imdecode(&buf, imgcodecs::IMREAD_COLOR)?;
            if frame.empty() {
                println!("Decoded empty frame, skipping");
                continue;
            }
            highgui::imshow("SRT Receiver", &frame)?;
            if highgui::wait_key(1)? == 27 {
                println!("ESC pressed, exiting");
                return Ok(());
            }
        }
    }

    Ok(())
}
