#![allow(clippy::unnecessary_wraps)]

use std::{
    io,
    net::UdpSocket,
    process::exit,
};

use crate::{
    frame_buffer::{FrameBuffer, GetFrameResult},
    comm::Actions,
    packet::Packet,
};
use ggez::{
    event,
    glam::*,
    graphics::{self, DrawParam, Drawable},
    Context, GameResult,
};
 

struct MainState {
    texture: Option<graphics::Image>,
    frames: FrameBuffer,
    socket: UdpSocket
}

impl MainState {
    fn new(socket:UdpSocket, _ctx: &mut Context) -> GameResult<MainState> {
        _ctx.gfx
            .set_resizable(true)
            .expect("Error setting window to resizable");
        _ctx.gfx.set_window_title("Screen Stream Client");

        

        Ok(MainState { 
            texture: None,
            frames: FrameBuffer::new(),
            socket
        })
    }
}

impl event::EventHandler<ggez::GameError> for MainState {
    fn quit_event(&mut self, _ctx: &mut Context) -> Result<bool, ggez::GameError> {
        // Send disconnection notification
        self.socket
            .send(&[Actions::Disconnection as u8])
            .expect("Error sending disconnection notification to server");

        return Ok(false);
    }

    fn update(&mut self, ctx: &mut Context) -> GameResult {
        // Check if stream is still open
        if self.socket.send(&[Actions::Ping as u8]).is_err() {
            println!("Stream is closed");
            exit(0);
        }

        // * Frame will be sent in packets of CHUNK_SIZE
        let mut buffer = [0u8; 65000 * 1];

        match self.socket.recv(&mut buffer) {
            Ok(bytes_read) => {
                println!("Bytes read: {}", bytes_read);

                // No bytes read means server closed the connection
                if bytes_read == 0 {
                    println!("Server closed the connection");
                    exit(0);
                }
                // If not even minimum bytes are read
                else if bytes_read < Packet::META_SIZE {
                    eprintln!("Invalid packet received, Expected at least: {} bytes, recieved: {}", Packet::META_SIZE, bytes_read);
                    return Ok(());
                }

                if bytes_read <= Packet::CHUNK_SIZE {
                    let packet = Packet::from_bytes(buffer[..bytes_read].to_vec());
                    // println!(
                    //     "Single packet received frame: {} index: {}",
                    //     packet.frame_id, packet.index
                    // );

                    self.frames.add_packet(packet);
                }

                // // * If multiple packets are received
                // // Split the buffer into packets
                // let mut index = 0;
                // while index < bytes_read {
                //     // At least meta size bytes are required
                //     if index + Packet::META_SIZE > bytes_read {
                //         break;
                //     }

                //     let end = std::cmp::min(index + Packet::CHUNK_SIZE, bytes_read);
                //     let packet = Packet::from_bytes(buffer[index..end].to_vec());

                //     self.frames.add_packet(packet);

                //     index = end;

                //     // If last packet is less than 4096 bytes
                //     if end == bytes_read {
                //         break;
                //     }
                // }
            }
            Err(e) => {
                match e.kind() {
                    io::ErrorKind::WouldBlock => {
                        // println!("No data available");
                    }
                    io::ErrorKind::ConnectionReset => {
                        println!("Connection reset by server");
                        exit(0);
                    }
                    _ => {
                        eprintln!("Error receiving data: {:?}", e);
                        exit(1);
                    }
                }
            }
        }

        // No frames -> return
        if self.frames.len() == 0 {
            return Ok(());
        }

        // println!("Frame buffer count: {}", self.frames.len());

        // Sort frames by frame_id (low to high)

        let buffer = match self.frames.get_frame() {
            GetFrameResult::NoFrame => {
                return Ok(());
            }

            GetFrameResult::NonSequential(packets) => {
                println!(
                    "Not sequential packet: {:?}",
                    packets.iter().map(|p| p.index).collect::<Vec<u8>>()
                );

                return Ok(());
            }

            GetFrameResult::Ok(buffer) => buffer,
        };

        // * Convert image to texture
        match graphics::Image::from_bytes(&ctx.gfx, &buffer) {
            Ok(texture) => {
                self.texture = Some(texture);
            }
            Err(e) => {
                eprintln!("Error converting image to texture: {:?}", e);
            }
        }

        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult {
        let mut canvas = graphics::Canvas::from_frame(ctx, graphics::Color::BLACK);

        // Display the image
        if let Some(texture) = &self.texture {
            // texture needs to fit the screen
            let (w, h) = ctx.gfx.size();

            let dest_point = Vec2::new(0.0, 0.0);
            // texture.draw(&mut canvas, DrawParam::new().dest(dest_point));

            texture.draw(
                &mut canvas,
                DrawParam::new()
                    .dest(dest_point)
                    .transform(ggez::mint::ColumnMatrix4 {
                        x: Vec4::new(w as f32 / texture.width() as f32, 0.0, 0.0, 0.0).into(),
                        y: Vec4::new(0.0, h as f32 / texture.height() as f32, 0.0, 0.0).into(),
                        z: Vec4::new(0.0, 0.0, 1.0, 0.0).into(),
                        w: Vec4::new(0.0, 0.0, 0.0, 1.0).into(),
                    }),
            );
        }

        canvas.finish(ctx)?;

        Ok(())
    }
}

pub fn run(address: String) -> GameResult {
    let cb: ggez::ContextBuilder = ggez::ContextBuilder::new("ss-client", "nova");
    let (mut ctx, event_loop) = cb.build()?;


    let socket: UdpSocket = UdpSocket::bind(format!("0.0.0.0:{}", 8899))
        .expect("Error binding to address");

    socket.set_nonblocking(true).expect("Error setting socket to non-blocking");

    socket
        .connect(&address)
        .expect("Error connecting to address");

    // 1 = Connection notification
    socket
        .send(&[Actions::NewConnection as u8])
        .expect("Error sending connection notification to server");

    println!("Connected to: {}", address);

    let state = MainState::new(socket, &mut ctx)?;

    event::run(ctx, event_loop, state);
}
