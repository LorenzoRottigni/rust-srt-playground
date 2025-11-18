use bytes::Bytes;
use futures::{stream, SinkExt, StreamExt};
use opencv::{
    core::Vector,
    imgcodecs,
    prelude::*,
    videoio::{VideoCapture, CAP_ANY},
};
use srt_tokio::SrtSocket;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Open default camera
    let cam = VideoCapture::new(0, CAP_ANY)?;
    if !VideoCapture::is_opened(&cam)? {
        panic!("Cannot open camera");
    }

    // Bind SRT socket
    let mut srt_socket = SrtSocket::builder().listen_on(":3333").await?;

    // Stream frames as bytes
    let mut stream = stream::unfold(cam, |mut cam| async move {
        let mut frame = Mat::default();
        if cam.read(&mut frame).unwrap() && !frame.empty() {
            // Encode frame as JPEG
            let mut buf = Vector::<u8>::new();
            imgcodecs::imencode(".jpg", &frame, &mut buf, &opencv::core::Vector::new()).unwrap();

            print!("\rSent frame");
            // sleep(Duration::from_millis(33)).await; // ~30 FPS

            Some((
                Ok((std::time::Instant::now(), Bytes::from(buf.to_vec()))),
                cam,
            ))
        } else {
            None
        }
    })
    .boxed();

    srt_socket.send_all(&mut stream).await?;
    Ok(())
}
