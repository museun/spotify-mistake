use std::future::Future;

use tokio::sync::oneshot;

pub fn defer_repaint(ctx: &egui::Context) -> impl Drop + '_ {
    struct Defer<'a>(&'a egui::Context);
    impl<'a> Drop for Defer<'a> {
        fn drop(&mut self) {
            self.0.request_repaint();
        }
    }
    Defer(ctx)
}

pub struct Fut<T> {
    pub fut: oneshot::Receiver<T>,
    pub resolved: bool,
}

impl<T> Default for Fut<T>
where
    T: Default,
{
    fn default() -> Self {
        let (tx, rx) = oneshot::channel();
        let _ = tx.send(T::default());
        Self {
            fut: rx,
            resolved: false,
        }
    }
}

impl<T> Fut<T>
where
    T: Send + Sync + 'static,
{
    pub fn spawn<F>(fut: F) -> Self
    where
        F: Future<Output = T> + Send + Sync + 'static,
    {
        let (tx, rx) = oneshot::channel();
        tokio::spawn(async move {
            let _ = tx.send(fut.await);
        });
        Self {
            fut: rx,
            resolved: false,
        }
    }

    pub fn resolve(&mut self) -> Option<T> {
        let item = self.fut.try_recv().ok()?;
        self.resolved = true;
        Some(item)
    }

    pub const fn is_resolved(&self) -> bool {
        self.resolved
    }
}

pub enum Ready<T> {
    Ready(T),
    NotReady,
}
