use tokio::sync::oneshot;

pub struct Cancel(Option<oneshot::Sender<()>>);

impl Cancel {
    pub fn new() -> (Self, oneshot::Receiver<()>) {
        let (s, r) = oneshot::channel();
        (Self(Some(s)), r)
    }
}

impl Drop for Cancel {
    fn drop(&mut self) {
        if let Some(c) = self.0.take() {
            let _ = c.send(());
        }
    }
}