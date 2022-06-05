// This currently doesn't benefit at all from being actory, 
// since all its operations are "ask me to do X, then I do X"

// NOTE: STD mutexes are not OK with async fns
// DirectoryData should not expose any

use std::{sync::{Arc, Mutex, Weak}, collections::HashMap};

use slotmap::{SlotMap, SecondaryMap};
use tokio::sync::mpsc;

use crate::{room::{RoomID, Room}, user::{UserID, User}, sock::Sock, protocol::{IRCString, ToUser}};

pub struct DirectoryRoot {
    data: Arc<Mutex<DirectoryData>>,
}

#[derive(Clone)]
pub struct Directory {
    data: Weak<Mutex<DirectoryData>>,
}

struct DirectoryData {
    rooms: SlotMap<RoomID, Room>,
    users: SlotMap<UserID, User>,

    users_by_nick: HashMap<IRCString, UserID>,
    user_nicks: SecondaryMap<UserID, IRCString>,
}

impl DirectoryRoot {
    pub fn new() -> Self {
        Self { 
            data: Arc::new(Mutex::new(DirectoryData::new()))
        }
    }

    pub fn share(&self) -> Directory {
        Directory { data: Arc::downgrade(&self.data) }
    }
}

pub enum ChangeNickError {
    NickInUse
}

impl Directory {
    pub fn user_create(&self, conn: Sock) -> Option<UserID> {
        let dir = self.clone();
        self.data.upgrade().map(|a| a.lock().unwrap().user_create(dir, conn))
    }

    pub fn user_drop(&self, user_id: UserID) {
        self.data.upgrade().map(|a| a.lock().unwrap().user_drop(user_id));
    }

    pub fn user_get_mailbox(&self, user_id: UserID) -> Option<mpsc::Sender<ToUser>> {
        self.data.upgrade().and_then(|a| a.lock().unwrap().user_get_mailbox(user_id))
    }

    pub fn user_by_nick(&self, nick: &IRCString) -> Option<UserID> {
        self.data.upgrade().and_then(|a| a.lock().unwrap().user_by_nick(nick))
    }

    pub fn user_get_nick(&self, user_id: UserID) -> Option<IRCString> {
        self.data.upgrade().and_then(|a| a.lock().unwrap().user_get_nick(user_id))
    }

    pub fn user_nick_to_mailbox(&self, nick: &IRCString) -> Option<mpsc::Sender<ToUser>> {
        self.data.upgrade().and_then(|a| {
            let a2 = a.lock().unwrap();
            if let Some(user_id) = a2.user_by_nick(nick) {
                a2.user_get_mailbox(user_id)
            } else {
                None
            }
        })
    }

    pub fn user_change_nick(&self, user_id: UserID, nick: Option<IRCString>) -> Result<(), ChangeNickError> {
        if let Some(dir) = self.data.upgrade() {
            dir.lock().unwrap().user_change_nick(user_id, nick)
        } else {
            // doesn't matter
            return Ok(())
        }
    }
}

impl DirectoryData {
    fn new() -> Self {
        Self {
            rooms: SlotMap::with_key(),
            users: SlotMap::with_key(),

            users_by_nick: HashMap::new(),
            user_nicks: SecondaryMap::new(),
        }
    }

    fn user_create(&mut self, dir: Directory, conn: Sock) -> UserID {
        self.users.insert_with_key(|uid| User::new(uid, conn, dir))
    }

    fn user_drop(&mut self, user_id: UserID) {
        self.users.remove(user_id);
    }

    fn user_get_mailbox(&self, user_id: UserID) -> Option<mpsc::Sender<ToUser>> {
        self.users.get(user_id).map(|x| x.get_mailbox())
    }

    fn user_by_nick(&self, nick: &IRCString) -> Option<UserID> {
        self.users_by_nick.get(nick).map(|x| *x)
    }

    fn user_get_nick(&self, user_id: UserID) -> Option<IRCString> {
        self.user_nicks.get(user_id).map(|x| x.clone())
    }

    fn user_change_nick(&mut self, user_id: UserID, new_nick: Option<IRCString>) -> Result<(), ChangeNickError> {
        // make sure this is needed
        if self.user_nicks.get(user_id) == new_nick.as_ref() { return Ok(()) }

        // figure out if we can get the new nick
        if let Some(n) = new_nick.as_ref() {
            if self.users_by_nick.contains_key(n) { return Err(ChangeNickError::NickInUse); }
        }

        // remove the user's old nick, if applicable
        let old_nick = self.user_nicks.remove(user_id);
        if let Some(n) = old_nick {
            assert_eq!(Some(user_id), self.users_by_nick.remove(&n));
            assert_eq!(Some(n), self.user_nicks.remove(user_id));
        }

        // use the new nick
        if let Some(n) = new_nick {
            assert_eq!(None, self.user_nicks.insert(user_id, n.clone()));
            assert_eq!(None, self.users_by_nick.insert(n, user_id));
        }

        Ok(())
    }
}