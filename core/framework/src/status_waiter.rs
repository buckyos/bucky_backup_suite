// Implement an event StatusWaiter, which contains a state (generic).
// It provides two interfaces:
// async wait(&self, status_tester: S: Fn(status) -> bool),
// which returns when the status makes the status_tester function return true;
// set_status(status),
// which updates the status and if this status causes any status_tester to return true,
// it will wake up the corresponding wait interface.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

#[derive(Clone)]
pub struct StatusWaiter<S: Clone> {
    status: Arc<Mutex<S>>,
    waiters: Arc<Mutex<Vec<WaiterFuture<S>>>>,
}

impl<S: Clone> StatusWaiter<S> {
    pub fn new(initial_status: S) -> (Status<S>, Waiter<S>) {
        let obj = StatusWaiter {
            status: Arc::new(Mutex::new(initial_status)),
            waiters: Arc::new(Mutex::new(Vec::new())),
        };

        (Status(obj.clone()), Waiter(obj))
    }
}

#[derive(Clone)]
pub struct Status<S: Clone>(StatusWaiter<S>);

impl<S: Clone> Status<S> {
    pub fn set(&self, new_status: S) {
        *self.0.status.lock().unwrap() = new_status.clone();
        let mut waiters = self.0.waiters.lock().unwrap();
        let mut woken_waiters = Vec::new();
        let mut retain_waiters = Vec::new();

        for waiter in waiters.iter() {
            if waiter.test(&new_status) {
                waiter.wake();
                woken_waiters.push(waiter.clone());
            } else {
                retain_waiters.push(waiter.clone());
            }
        }

        *waiters = retain_waiters;
    }
}

#[derive(Clone)]
pub struct Waiter<S: Clone>(StatusWaiter<S>);

impl<S: Clone> Waiter<S> {
    pub fn wait<F>(&self, status_tester: F) -> WaiterFuture<S>
    where
        F: Fn(&S) -> bool + Send + Sync + 'static,
    {
        let waiter = WaiterFuture::new(status_tester, self.0.status.clone());
        self.0.waiters.lock().unwrap().push(waiter.clone());

        waiter
    }
}

pub trait SpecificStatusWaiter {
    type StatusType: Clone + std::cmp::Eq;
    fn wait_status(&self, status: &[Self::StatusType]) -> WaiterFuture<Self::StatusType>;
}

impl<PS: Clone + std::cmp::Eq> SpecificStatusWaiter for Waiter<PS> {
    type StatusType = PS;
    fn wait_status(&self, status_slice: &[PS]) -> WaiterFuture<PS> {
        self.wait(|status| status_slice.iter().find(|s| *s == status).is_some())
    }
}

struct WaiterFutureImpl<S: Clone> {
    status: Arc<Mutex<S>>,
    status_tester: Box<dyn Fn(&S) -> bool + Send + Sync + 'static>,
    waker: Mutex<Option<std::task::Waker>>,
}

#[derive(Clone)]
pub struct WaiterFuture<S: Clone>(Arc<WaiterFutureImpl<S>>);

impl<S: Clone> WaiterFuture<S> {
    fn new<F>(status_tester: F, status: Arc<Mutex<S>>) -> Self
    where
        F: Fn(&S) -> bool + Send + Sync + 'static,
    {
        WaiterFuture(Arc::new(WaiterFutureImpl {
            status,
            status_tester: Box::new(status_tester),
            waker: Mutex::new(None),
        }))
    }

    fn test(&self, status: &S) -> bool {
        (self.0.status_tester)(status)
    }

    fn wake(&self) {
        let waker = self.0.waker.lock().unwrap().clone();
        if let Some(waker) = waker {
            waker.wake();
        }
    }
}

impl<S: Clone> Future for WaiterFuture<S> {
    type Output = S;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        {
            let status = self.0.status.lock().unwrap();
            if self.test(&*status) {
                return Poll::Ready(status.clone());
            }
        }

        let waker = cx.waker().clone();
        *self.0.waker.lock().unwrap() = Some(waker);
        Poll::Pending
    }
}
