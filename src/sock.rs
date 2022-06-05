use std::{net::SocketAddr};

use tokio::{net::{TcpStream, tcp::{OwnedReadHalf, OwnedWriteHalf}}, io::{AsyncReadExt, AsyncWriteExt}, sync::{mpsc::{UnboundedSender, UnboundedReceiver}, oneshot}, time::Instant};
use tokio::sync::mpsc;

use crate::{cancel::Cancel, protocol::IRCString};

pub struct Sock {
    addr: SocketAddr, 
    pub recv: UnboundedReceiver<MessageIn>,
    pub send: UnboundedSender<MessageOut>,
    cancel1: Cancel,
    cancel2: Cancel,
}

impl Sock {
    pub fn watch(socket: TcpStream, addr: SocketAddr) -> Sock {
        let (r, w) = socket.into_split();

        let (cancel1, receive_cancel1) = Cancel::new();
        let (cancel2, receive_cancel2) = Cancel::new();

        let recv ={
            let (tx, rx) =  mpsc::unbounded_channel();
            tokio::spawn(async { Sock::_read(r, tx, receive_cancel1).await });
            rx
        };
        let send = {
            let (tx, rx) =  mpsc::unbounded_channel();
            tokio::spawn(async { Sock::_write(w, rx, receive_cancel2).await });
            tx
        };

        Sock { addr, recv, send, cancel1, cancel2 }
    }

    async fn _read(mut read: OwnedReadHalf, tx: UnboundedSender<MessageIn>, mut cancel: oneshot::Receiver<()>) {
        let mut buf = [0; 512];
        let mut msg_in_progress = Vec::with_capacity(512);

        loop {
            let n = tokio::select! {
                _ = &mut cancel => { println!("socket dropped"); return }
                x = read.read(&mut buf) => match x {
                    Ok(n) if n == 0 => { 
                        eprintln!("user quit: done");
                        return;
                    }
                    Ok(n) => { n }
                    Err(e) => {
                        eprintln!("failed to read from socket; err = {:?}", e);
                        return;
                    }
                }
            };

            for i in 0..n {
                msg_in_progress.push(buf[i]);
                if msg_in_progress.len() > 512 {
                    eprintln!("message too long");
                    return;
                }

                if msg_in_progress.ends_with(b"\r\n") {
                    // get rid of the terminator
                    msg_in_progress.pop();
                    msg_in_progress.pop();

                    match tx.send(MessageIn { time: Instant::now(), data: IRCString::new(msg_in_progress) }) {
                        Ok(()) => {}
                        Err(e) => {
                            eprintln!("failed to send to channel; err = {:?}", e);
                        }
                    }
                    msg_in_progress = Vec::with_capacity(512);
                }
            }
        }
    }

    async fn _write(mut write: OwnedWriteHalf, mut tx: UnboundedReceiver<MessageOut>, cancel: oneshot::Receiver<()>) {
        let mut done = false;
        let mut send_at: Option<Instant> = None;
        let mut write_buf = vec![];

        tokio::pin!(cancel);

        loop {
            let now = Instant::now();
            match send_at {
                None => {
                    if done { return; }
                    // nothing to send
                    tokio::select! {
                        _ = &mut cancel => { return; }
                        x = tx.recv() => match x {
                            Some(msg) => {
                                send_at = Some(msg.deadline);
                                write_buf.extend(msg.data.bytes)
                            }
                            None => { done = true; },  
                        }
                    }
                }
                Some(sa) if done || sa <= now => {
                    match write.write_all(&write_buf).await {
                        Ok(()) => {}
                        Err(e) => {
                            eprintln!("failed to send to user: {:?} {:?}", write_buf, e);
                            return;
                        }
                    }
                    send_at = None;
                    write_buf.clear();
                }
                Some(sa) => {
                    // now figure out what to do
                    let sleep = tokio::time::sleep_until(sa);
                    tokio::pin!(sleep);

                    tokio::select! {
                        _ = &mut cancel => { return }
                        _ = &mut sleep => {
                            // loop around again
                        }
                        x = tx.recv() => {
                            match x {
                                Some(msg) => {
                                    send_at = Some(sa.min(msg.deadline));
                                    write_buf.extend(msg.data.bytes);
                                }
                                None => { done = true; } // force any current messages to be sent
                            }
                        }
                    }
                }
            }
        }
    }
}

pub struct MessageIn {
    pub time: Instant,
    pub data: IRCString,
}

#[derive(Debug)]
pub struct MessageOut {
    pub deadline: Instant,
    pub data: IRCString,
}

impl std::fmt::Debug for MessageIn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO: Don't allocate here
        f.debug_struct("MessageIn").field("data", &std::str::from_utf8(&self.data.bytes)).finish()
    }
}