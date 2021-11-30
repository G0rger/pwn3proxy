use tokio::sync::{mpsc, watch};

#[derive(Debug, Clone)]
pub struct Shutdown {
    notify: watch::Receiver<()>,
    shutdown: bool,
    running: mpsc::Sender<()>,
}

impl Shutdown {
    pub fn create() -> (watch::Sender<()>, mpsc::Receiver<()>, Self) {
        let (running_send, running_recv) = mpsc::channel(1);
        let (notify_send, notify_recv) = watch::channel(());
        (notify_send, running_recv, Self::new(notify_recv, running_send))
    }

    pub fn new(notify: watch::Receiver<()>, running: mpsc::Sender<()>) -> Self {
        Self {
            notify,
            running,
            shutdown: false
        }
    }

    // pub fn is_shutdown(&self) -> bool {
    //     self.shutdown
    // }

    pub async fn wait(&mut self) {
        if self.shutdown {
            return;
        }

        let _ = self.notify.changed().await;
        self.shutdown = true;
    }
}
