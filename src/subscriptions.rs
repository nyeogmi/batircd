/*
use std::{collections::HashMap, f32::consts::PI};
use std::hash::Hash;

use tokio::{sync::{broadcast, mpsc, oneshot}, task::JoinHandle, select};

pub struct Subscriptions {
    tx: mpsc::Sender<(String, Notification)>,
    members: HashMap<String, Subscription>
}

impl Subscriptions {
    pub fn new(tx: mpsc::Sender<(String, Notification)>) -> Self {
        Subscriptions { 
            tx, members: HashMap::new() 
        }
    }

    pub fn add(&mut self, key: String, mut receiver: broadcast::Receiver<Notification>) {
        let tx = self.tx.clone();

        self.remove(&key);

        let (cancel, mut read_cancel) = oneshot::channel();
        let kc = key.clone();
        let task = tokio::spawn(async move {
            loop {
                select!{
                    _ = &mut read_cancel => { return }
                    rcv = receiver.recv() => match rcv {
                        Ok(x) => { 
                            match tx.send((kc.clone(), x)).await {
                                Ok(()) => {}
                                Err(_) => { return }
                            } }
                        Err(_) => { return }
                    }
                };
            }
        });
        
        assert!(self.members.insert(key, Subscription { task, cancel }).is_none())
    }

    pub fn remove(&mut self, key: &str) {
        if let Some(old) = self.members.remove(key) { old.cancel() }
    }
}

struct Subscription {
    task: JoinHandle<()>,
    cancel: oneshot::Sender<()>,
}

impl Subscription {
    fn cancel(self) {
        match self.cancel.send(()) {
            Ok(()) => {}
            Err(()) => { /* it's ok if the other side hung up */ }
        }
    }
} */