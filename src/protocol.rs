use tokio::sync::mpsc;

use crate::user::UserID;
use crate::room::RoomID;

// A Vec<u8> that might be a valid string in UTF-8, but no one should bet on that.
// (IRC operates on bytestrings, not UTF-8 strings.)
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IRCString { pub bytes: Vec<u8> }

impl IRCString {
    pub fn new(bytes: Vec<u8>) -> Self {
        IRCString { bytes }
    }

    pub fn upper_inplace(&mut self) {
        for i in self.bytes.iter_mut() {
            if i.is_ascii_lowercase() { *i = i.to_ascii_uppercase() }
        }
    }
}

impl std::fmt::Debug for IRCString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match std::str::from_utf8(&self.bytes) {
            Ok(u) => u.fmt(f),
            Err(_) => "<invalid UTF8>".fmt(f),
        }
    }
}

#[derive(Debug)]
pub struct Command {
    pub pfx: Option<IRCString>,
    pub cmd: IRCString,
    pub args: Vec<IRCString>,
}

#[derive(Clone)]
pub enum U2R {  // user to room
    Kill {},
    Join { 
        user: UserID,
        user_mailbox: mpsc::Sender<(RoomID, R2U)>,
    },
    Part { user: UserID },
    Privmsg {
        user: UserID,
        message: IRCString,
    }
} 

#[derive(Clone)]
pub enum R2U {
    Join { user: UserID },
    Part { user: UserID },
    Privmsg {
        user: UserID,
        message: IRCString,
    }
}

pub enum U2U {
    Privmsg { message: IRCString }
}

pub enum ToUser {
    Room { room_id: RoomID, message: R2U },
    User { nick: IRCString, message: U2U }
}