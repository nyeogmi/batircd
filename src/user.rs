use std::collections::HashMap;

use slotmap::new_key_type;
use tokio::{sync::{mpsc, oneshot}, time::Instant};

use crate::{room::RoomID, protocol::{R2U, U2R, IRCString, Command, ToUser, U2U}, cancel::Cancel, sock::{Sock, MessageIn, MessageOut}, parse, directory::Directory};

new_key_type! { pub struct UserID; }

pub struct User {
    id: UserID, mailbox: mpsc::Sender<ToUser>,

    cancel: Cancel,
}

impl User {
    pub fn get_mailbox(&self) -> mpsc::Sender<ToUser> {
        self.mailbox.clone()
    }
}

pub struct UserState {
    id: UserID, mailbox: mpsc::Sender<ToUser>,
    receive_cancel: oneshot::Receiver<()>,
    done: bool,
    directory: Directory,

    sock: Sock,
    ingoing: mpsc::Receiver<ToUser>,


    id_card: UserIDCard,

    memberships: HashMap<RoomID, Membership>,
}

#[derive(Debug)]
pub struct UserIDCard {
    nick: Option<IRCString>,
    user: Option<IRCString>,
    realname: Option<IRCString>,
}


pub struct Membership {
    mailbox: mpsc::Sender<U2R>,
}

impl User {
    pub fn new(id: UserID, sock: Sock, directory: Directory) -> Self {
        let (mailbox, ingoing) = mpsc::channel(1);
        let (cancel, receive_cancel) = Cancel::new();

        let user_state = UserState {
            id, mailbox: mailbox.clone(),
            receive_cancel,
            done: false,
            directory,

            sock,
            ingoing,

            id_card: UserIDCard { nick: None, user: None, realname: None },

            memberships: HashMap::new(),
        };

        tokio::spawn(async { user_state.flow().await });

        return User { 
            id, mailbox,
            cancel
        }
    }
}

impl UserState {
    fn my_nick(&self) -> IRCString {
        self.id_card.nick.as_ref().map(|x| x.clone()).unwrap_or_else(|| IRCString::new(b"unknown".to_vec()))
    }
    async fn flow(mut self) {
        loop {
            if self.done { self.kill().await; return }

            tokio::select! {
                _ = &mut self.receive_cancel => { self.done = true; continue; },
                tcp = self.sock.recv.recv() => match tcp {
                    Some(t) => { 
                        let cmd = match parse::parse(&t) {
                            Some(cmd) => cmd,
                            None => {
                                eprintln!("couldn't parse: {:?}", t);
                                continue
                            }
                        };

                        println!("received: {:?}", cmd);

                        if !self.id_card.is_complete() {
                            self.handle_user_prelogin(cmd).await;
                        } else {
                            self.handle_user(cmd).await;
                        }

                    }
                    None => { self.done = true; continue; }
                },
                msg = self.ingoing.recv() => match msg {
                    Some(m) => {
                        self.handle_server(m).await;
                    }
                    None => { self.done = true; continue; }
                }
            }
        }
    }

    async fn handle_user_prelogin(&mut self, cmd: Command) {
        assert!(!self.id_card.is_complete());
        match (cmd.cmd.bytes.as_slice(), cmd.args.as_slice()) {
            (b"CAP", _) => { /* do nothing, we don't support capability negotiation */ }
            (b"NICK", [name]) => { 
                self.id_card.nick.replace(name.clone()); 
            }
            (b"USER", [user, _, _, realname]) => { 
                self.id_card.user.replace(user.clone());
                self.id_card.realname.replace(realname.clone());
            }
            _ => { panic!("TODO"); }
        }

        if self.id_card.is_complete() {
            match self.directory.user_change_nick(self.id, self.id_card.nick.clone()) {
                Ok(()) => { /* we're good */ }
                Err(_) => {
                    // TODO: Send a message
                    self.id_card.nick = None;
                    return
                }
            }

            let _ = self.sock.send.send(MessageOut { 
                deadline: Instant::now(), 
                data: IRCString::new(b"001 Nyeogmi :Welcome to the server (test)\r\n".to_vec())
            });
            let _ = self.sock.send.send(MessageOut { 
                deadline: Instant::now(), 
                data: IRCString::new(b"002 barf\r\n".to_vec())
            });
            let _ = self.sock.send.send(MessageOut { 
                deadline: Instant::now(), 
                data: IRCString::new(b"003 barf\r\n".to_vec())
            });
            let _ = self.sock.send.send(MessageOut { 
                deadline: Instant::now(), 
                data: IRCString::new(b"004 barf\r\n".to_vec())
            });
        } 
    }

    async fn handle_user(&mut self, cmd: Command) {
        match (cmd.cmd.bytes.as_slice(), cmd.args.as_slice()) {
            (b"PRIVMSG", [name, msg]) => {
                if name.bytes.starts_with(b"#")  {
                    panic!("TODO")
                } else {
                    match self.directory.user_nick_to_mailbox(name) {
                        Some(mb) => { 
                            match mb.send(ToUser::User { nick: self.my_nick(), message: U2U::Privmsg { 
                                message: msg.clone(),
                            }}).await {
                                Ok(()) => { /* */ }
                                Err(_) => { panic!("TODO") }
                            }
                        }
                        None => panic!("TODO")
                    }
                }
            }
            _ => { panic!("TODO") }
        }
    }

    async fn handle_server(&mut self, msg: ToUser) {
        match msg {
            ToUser::User { nick, message } => {
                match message {
                    U2U::Privmsg { message } => {
                        let _ = self.sock.send.send(parse::dump(Command { 
                            pfx: Some(nick),
                            cmd: IRCString::new(b"PRIVMSG".to_vec()),
                            args: vec![self.my_nick(), message]
                        }, 0.5));
                    }
                }
            }
            _ => { panic!("TODO") }
        }
    }

    pub fn send_room(&mut self, room: RoomID, msg: U2R) {
        panic!("TODO");
    }

    pub async fn kill(&mut self) {
        let rooms: Vec<RoomID> = self.memberships.keys().cloned().collect();  // TODO: Avoid this
        for room_id in rooms {
            self.send_room(room_id, U2R::Part { user: self.id })
        }
        self.memberships.clear();
        self.done = true;
    }
}
impl UserIDCard {
    pub(crate) fn is_complete(&self) -> bool {
        // we don't care about realname
        return self.nick.is_some() && self.user.is_some()
    }
}