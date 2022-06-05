use std::{collections::HashMap, ops::Not};

use slotmap::{SlotMap, Key};
use tokio::{sync::{broadcast, mpsc::{UnboundedReceiver, UnboundedSender}}, net::TcpListener};

use crate::{room::{Room, RoomID}, user::{User, UserID}, sock::Sock, directory::{Directory, DirectoryRoot}};

// use crate::{user_conn::{MessageOut, MessageIn}, subscriptions::{Subscriptions, Notification}};

/* 
struct DSlotMap<K: Key, V> {
    items: SlotMap<K, V>
}

impl<K: Key, V> DSlotMap<K, V> {
    fn new() -> DSlotMap<K, V> {
        DSlotMap { 
            items: SlotMap::with_key()
        }
    }

    fn insert(&mut self, v: impl FnOnce(K) -> V) -> K {
        self.items.insert_with_key(f)
    }
}

*/

pub struct World {
    directory_root: DirectoryRoot,
}

impl World {
    pub fn new() -> Self {
        World {
            directory_root: DirectoryRoot::new()
        }
    }

    fn directory(&self) -> Directory {
        self.directory_root.share()
    }

    pub async fn main_loop(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind("127.0.0.1:6667").await?;

        loop {
            let (socket, addr) = listener.accept().await?;
            println!("accepted!");

            self.directory().user_create(Sock::watch(socket, addr));
            println!("user created!");
        }
    }
}
