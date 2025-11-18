#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use std::{
        io::{self, Read, Write},
        time::{Duration, Instant},
    };

    use ac_ffmpeg::{
        format::{
            demuxer::Demuxer,
            io::IO,
            muxer::{Muxer, OutputFormat},
        },
        time::Timestamp,
    };
    use bytes::Bytes;
    use futures::SinkExt;
    use opencv::{core::Vector, imgcodecs, prelude::*, videoio};
    use srt_tokio::SrtSocket;
    use tokio::{
        runtime::Handle,
        sync::mpsc::{channel, Sender},
        time::sleep_until,
    };
    use tokio_stream::StreamExt;

    // ===================== WriteBridge =====================
    struct WriteBridge(tokio::sync::mpsc::Sender<(Instant, Bytes)>);
    impl Write for WriteBridge {
        fn write(&mut self, w: &[u8]) -> Result<usize, std::io::Error> {
            // Send the whole buffer as a single packet
            let bytes = Bytes::copy_from_slice(w);
            let sender = &mut self.0;
            // Use blocking send with tokio
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current()
                    .block_on(async { sender.send((Instant::now(), bytes)).await })
            })
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            Ok(w.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    pretty_env_logger::init();

    println!("Opening camera...");
    let cam = videoio::VideoCapture::new(0, videoio::CAP_ANY)?;
    if !videoio::VideoCapture::is_opened(&cam)? {
        panic!("Failed to open camera");
    }

    // ===================== CameraReader =====================
    struct CameraReader {
        cam: videoio::VideoCapture,
        buffer: Vec<u8>,
    }

    impl CameraReader {
        fn new(cam: videoio::VideoCapture) -> Self {
            Self {
                cam,
                buffer: Vec::new(),
            }
        }
    }

    impl Read for CameraReader {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.buffer.is_empty() {
                let mut frame = Mat::default();
                self.cam
                    .read(&mut frame)
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

                if frame.empty() {
                    // <- just remove `?`
                    return Ok(0);
                }

                let mut encoded = Vector::<u8>::new();
                let mut params = Vector::<i32>::new();
                params.push(imgcodecs::IMWRITE_JPEG_QUALITY);
                params.push(80);

                imgcodecs::imencode(".jpg", &frame, &mut encoded, &params)
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                self.buffer = encoded.to_vec();
            }

            let len = buf.len().min(self.buffer.len());
            buf[..len].copy_from_slice(&self.buffer[..len]);
            self.buffer.drain(..len);
            Ok(len)
        }
    }

    // ===================== Build Demuxer =====================
    let reader = CameraReader::new(cam);
    let io = IO::from_read_stream(reader);

    let mut demuxer = Demuxer::builder()
        .build(io)?
        .find_stream_info(None)
        .map_err(|(_, err)| err)?;

    for (index, stream) in demuxer.streams().iter().enumerate() {
        let params = stream.codec_parameters();

        println!("Stream #{index}:");
        println!("  duration: {}", stream.duration().as_f64().unwrap_or(0f64));

        if let Some(params) = params.as_audio_codec_parameters() {
            println!("  type: audio");
            println!("  codec: {}", params.decoder_name().unwrap_or("N/A"));
            println!("  sample format: {}", params.sample_format().name());
            println!("  sample rate: {}", params.sample_rate());
            println!("  channels: {}", params.channel_layout().channels());
        } else if let Some(params) = params.as_video_codec_parameters() {
            println!("  type: video");
            println!("  codec: {}", params.decoder_name().unwrap_or("N/A"));
            println!("  width: {}", params.width());
            println!("  height: {}", params.height());
            println!("  pixel format: {}", params.pixel_format().name());
        } else {
            println!("  type: unknown");
        }
    }

    println!("Waiting for a connection to start streaming...");

    let mut socket = SrtSocket::builder()
        .latency(Duration::from_millis(1000))
        .listen_on(":1234")
        .await?;

    println!("Connection established");

    let mut last_pts_inst: Option<(Timestamp, Instant)> = None;
    let (chan_send, chan_recv) = channel(1024);

    // ===================== Demuxer Task =====================
    let demuxer_task = tokio::spawn(async move {
        let streams = demuxer
            .streams()
            .iter()
            .map(|stream| stream.codec_parameters())
            .collect::<Vec<_>>();

        let io = IO::from_write_stream(WriteBridge(chan_send));

        let mut muxer_builder = Muxer::builder();
        for codec_parameters in streams {
            muxer_builder.add_stream(&codec_parameters).unwrap();
        }

        let mut muxer = muxer_builder
            .build(io, OutputFormat::find_by_name("mpegts").unwrap())
            .unwrap();

        while let Some(packet) = demuxer.take().unwrap() {
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

            println!(
                "Sending packet {:?} len={}",
                packet.pts(),
                packet.data().len()
            );

            muxer.push(packet).unwrap();
        }
    });

    let mut stream = tokio_stream::wrappers::ReceiverStream::new(chan_recv).map(Ok::<_, io::Error>);
    socket.send_all(&mut stream).await?;
    socket.close().await?;

    demuxer_task.await?;

    Ok(())
}
