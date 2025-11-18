use std::{
    io::{self, Write},
    time::{Duration, Instant},
};

use ac_ffmpeg::{
    codec::{video::VideoEncoder, Codec},
    format::{
        io::IO,
        muxer::{Muxer, OutputFormat},
    },
    time::Timestamp,
};
use bytes::Bytes;
use futures::SinkExt;
use opencv::{
    core::{Mat, Size},
    prelude::*,
    videoio::{VideoCapture, VideoCaptureTrait, CAP_ANY},
};
use srt_tokio::SrtSocket;
use tokio::{
    runtime::Handle,
    sync::mpsc::{channel, Sender},
    time::sleep_until,
};
use tokio_stream::StreamExt;

struct WriteBridge(Sender<(Instant, Bytes)>);

impl Write for WriteBridge {
    fn write(&mut self, w: &[u8]) -> Result<usize, std::io::Error> {
        for chunk in w.chunks(1316) {
            if self
                .0
                .try_send((Instant::now(), Bytes::copy_from_slice(chunk)))
                .is_err()
            {
                println!("Sender throttled, dropping packet");
            }
        }
        Ok(w.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Handle::current().block_on(async { Ok(()) })
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    // --- OpenCV camera ---
    let mut cam = VideoCapture::new(0, CAP_ANY)?; // device 0
    if !cam.is_opened()? {
        panic!("Cannot open camera");
    }

    let width = cam.get(opencv::videoio::CAP_PROP_FRAME_WIDTH)? as usize;
    let height = cam.get(opencv::videoio::CAP_PROP_FRAME_HEIGHT)? as usize;
    let fps = cam.get(opencv::videoio::CAP_PROP_FPS)? as i32;
    println!("Camera opened: {}x{} @ {}fps", width, height, fps);

    // --- SRT setup ---
    println!("Waiting for a connection...");
    let mut socket = SrtSocket::builder()
        .latency(Duration::from_millis(1000))
        .listen_on(":1234")
        .await?;
    println!("Connection established");

    let (chan_send, chan_recv) = channel(1024);
    let io_bridge = IO::from_write_stream(WriteBridge(chan_send));

    // --- Encoder setup ---
    let mut encoder = VideoEncoder::builder()
        .codec(Codec::H264)
        .width(width)
        .height(height)
        .fps(fps)
        .time_base((1, fps))
        .build()?;

    let mut muxer_builder = Muxer::builder();
    muxer_builder.add_stream(&encoder.codec_parameters())?;
    let mut muxer =
        muxer_builder.build(io_bridge, OutputFormat::find_by_name("mpegts").unwrap())?;

    // --- Camera loop ---
    let demuxer_task = tokio::spawn(async move {
        let mut last_pts_inst: Option<(Timestamp, Instant)> = None;

        loop {
            let mut frame = Mat::default();
            if !cam.read(&mut frame).unwrap_or(false) || frame.empty().unwrap_or(true) {
                tokio::time::sleep(Duration::from_millis(10)).await;
                continue;
            }

            // Encode frame
            let packet = encoder.encode(&frame).unwrap();
            let pts = packet.pts();
            let _inst = match last_pts_inst {
                Some((last_pts, last_inst)) => {
                    if pts < last_pts {
                        last_inst
                    } else {
                        let d_t = pts - last_pts;
                        let deadline = last_inst + d_t;
                        sleep_until(deadline.into()).await;
                        last_pts_inst = Some((pts, deadline));
                        deadline
                    }
                }
                None => {
                    let now = Instant::now();
                    last_pts_inst = Some((pts, now));
                    now
                }
            };

            muxer.push(&packet).unwrap();
            println!("Sent packet pts={:?} len={}", pts, packet.data().len());
        }
    });

    let mut stream = tokio_stream::wrappers::ReceiverStream::new(chan_recv).map(Ok::<_, io::Error>);
    socket.send_all(&mut stream).await?;
    socket.close().await?;

    demuxer_task.await?;

    Ok(())
}
