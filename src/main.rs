extern crate futures;
extern crate tokio_core;
#[macro_use]
extern crate tokio_io;

use futures::{Future, Stream, Poll};
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_core::net::TcpListener;
use tokio_core::reactor::Core;

use std::io as stdio;

#[derive(Debug)]
struct Score<R, W> {
    reader: Option<R>,
    read_done: bool,
    writer: Option<W>,
    pos: usize,
    cap: usize,
    buf: Box<[u8]>,
    score: u64,
}

fn score<R, W>(reader: R, writer: W) -> Score<R, W>
    where R: AsyncRead,
          W: AsyncWrite
{
    Score {
        reader: Some(reader),
        read_done: false,
        writer: Some(writer),
        pos: 0,
        cap: 0,
        buf: Box::new([0; 2048]),
        score: 0u64,
    }
}

impl<R, W> Future for Score<R, W>
    where R: AsyncRead,
          W: AsyncWrite
{
    type Item = (u64, R, W);
    type Error = stdio::Error;

    fn poll(&mut self) -> Poll<(u64, R, W), stdio::Error> {
        loop {
            if self.pos == self.cap && !self.read_done {
                let reader = self.reader.as_mut().unwrap();
                let n = try_nb!(reader.read(&mut self.buf));
                if n == 0 {
                    self.read_done = true;
                } else {
                    self.pos = 0;
                    self.cap = n;
                }
            }

            while self.pos < self.cap {
                self.score += self.buf[self.pos] as u64;
                self.pos += 1;
            }

            if self.pos == self.cap && self.read_done {
                let score_str = self.score.to_string();
                let score_bytes = score_str.as_bytes();
                let mut wpos: usize = 0;
                while wpos < score_bytes.len() {
                    let writer = self.writer.as_mut().unwrap();
                    let i = try_nb!(writer.write(&score_bytes[wpos..score_bytes.len()]));
                    wpos += i;
                }
                {
                    let writer = self.writer.as_mut().unwrap();
                    try_nb!(writer.write(&['\n' as u8]));
                }
                try_nb!(self.writer
                            .as_mut()
                            .unwrap()
                            .flush());
                let reader = self.reader.take().unwrap();
                let writer = self.writer.take().unwrap();
                return Ok((wpos as u64, reader, writer).into());
            }
        }
    }
}

fn main() {
    let mut ev_loop = Core::new().unwrap();
    let handle = ev_loop.handle();
    let mut counter = 0u64;

    let addr = "127.0.0.1:12345".parse().unwrap();
    let tcp = TcpListener::bind(&addr, &handle).unwrap();

    let server = tcp.incoming().for_each(|(client_stream, client_sock)| {
        print!("New Connection from: {} ", &client_sock);
        counter = counter + 1u64;
        let (reader, writer) = client_stream.split();

        let s = score(reader, writer);
        let score_handle =
            s.map(|(n, _, _)| println!("wrote {} bytes", n)).map_err(|err| {
                                                                         println!("IO error: {:?}",
                                                                                  err)
                                                                     });

        handle.spawn(score_handle);

        if counter % 100 == 0 {
            println!("Total connections: {}", counter);
        }

        Ok(())
    });

    ev_loop.run(server).unwrap();
}
