use std::collections::HashMap;

use slotmap::new_key_type;
use tokio::{sync::{broadcast, oneshot, mpsc, watch}, spawn};

use crate::{cancel::Cancel, protocol::{R2U, U2R}, user::UserID};

new_key_type! { pub struct RoomID; }

pub struct Room {
    id: RoomID, mailbox: mpsc::Sender<U2R>,
    cancel: Cancel,

    snapshot: watch::Receiver<RoomSnapshot>,
}

pub struct RoomState {
    id: RoomID, mailbox: mpsc::Sender<U2R>,
    receive_cancel: oneshot::Receiver<()>,
    done: bool,

    // TODO: A layer of indirection between the mailbox and the users.
    // The channel coroutine should contain its own private state
    ingoing: mpsc::Receiver<U2R>,
    outgoing: broadcast::Sender<R2U>,
    snapshot: watch::Sender<RoomSnapshot>,

    members: HashMap<UserID, Member>,
}

#[derive(Clone)]
pub struct RoomSnapshot {
    pub n_members: usize,
}

struct Member {
    cancel: Cancel,
    mailbox: mpsc::Sender<(RoomID, R2U)> 
}

impl Room {
    pub fn new(id: RoomID) -> Self {
        let (mailbox, ingoing) = mpsc::channel(1);
        let (outgoing, _) = broadcast::channel(256);
        let (cancel, receive_cancel) = Cancel::new();
        let (set_snapshot, receive_snapshot) = watch::channel(RoomSnapshot { n_members: 0});

        let room_state = RoomState { 
            id, mailbox: mailbox.clone(),
            receive_cancel,
            done: false,

            ingoing,
            outgoing,
            snapshot: set_snapshot,

            members: HashMap::new()
        };

        tokio::spawn(async { room_state.flow() });

        return Room {
            id, mailbox,
            cancel,

            snapshot: receive_snapshot,
        }
    }
}

impl RoomState {
    async fn flow(mut self) {
        loop {
            self.touch_snapshot();

            if self.done { self.kill().await; return }
            let u2r = tokio::select! {
                _ = &mut self.receive_cancel => { self.done = true; continue; }
                n = self.ingoing.recv() => match n {
                    Some(x) => { x }
                    None => { return }
                }
            };

            match u2r {
                U2R::Kill { } => { self.done = true; }
                U2R::Join { user, user_mailbox } => { self.join(user, user_mailbox).await }
                U2R::Part { user } => { self.part(user).await }
                _ => { panic!("TODO"); }
            }
        }
    }

    fn touch_snapshot(&mut self) {
        let _ = self.snapshot.send(RoomSnapshot { n_members: self.members.len() });
    }

    async fn send(&mut self, user_id: UserID, msg: R2U) {
        if let Some(member) = self.members.get(&user_id) {
            match member.mailbox.send((self.id, msg)).await {
                Ok(()) => {}
                Err(_) => { /* TODO: Do we care? */ }
            }
        }
    }

    async fn broadcast(&mut self, msg: R2U) {
        match self.outgoing.send(msg) {
            Ok(_) => {}
            Err(_) => { self.done = true }
        }
    }

    pub async fn kill(&mut self) {
        let members: Vec<UserID> = self.members.keys().cloned().collect();  // TODO: Avoid this
        for user in members {
            self.broadcast(R2U::Part { user }).await;
        }
        self.members.clear();
        self.done = true;
    }

    pub async fn join(&mut self, user: UserID, mailbox: mpsc::Sender<(RoomID, R2U)>) {
        // send messages from channel to user 
        let (cancel, receive_cancel) = Cancel::new();

        if self.members.contains_key(&user) { return; }

        let me_id = self.id;
        let from_me = self.outgoing.subscribe();
        let to_user = mailbox.clone();
        spawn(async move {
            tokio::pin!(receive_cancel);
            tokio::pin!(from_me);
            loop {
                tokio::select! {
                    _ = &mut receive_cancel => { return }
                    x = from_me.recv() => match x {
                        Ok(msg) => { 
                            match to_user.send((me_id, msg)).await {
                                Ok(()) => (),
                                Err(_) => { return }
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => {
                            // well that sucks. right now do nothing
                            eprintln!("lagged!");
                        }
                        Err(broadcast::error::RecvError::Closed) => { return }
                    }
                };
            }
        });

        assert!(self.members.insert(user, Member {cancel, mailbox}).is_none());

        self.broadcast(R2U::Join { user }).await
    }

    pub async fn part(&mut self, user: UserID) {
        if !self.members.contains_key(&user) { return; }
        self.broadcast(R2U::Part { user }).await;
        assert!(self.members.remove(&user).is_some());
    }
}