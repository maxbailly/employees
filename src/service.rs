/// A type that implements this trait can open an communication channel between the type and some other type
/// implements the [`Connect`] trait of the type.
///
/// # Examples
///
/// ```
/// # use employees::*;
/// # use std::sync::mpsc::{self, Receiver, Sender};
/// #[derive(Default)]
/// struct Producer {
///     sender: Option<Sender<u8>>,
/// }
///
/// impl Connect<Consumer> for Producer {
///     fn on_connection(&mut self, endpoint: Sender<u8>) {
///         self.sender = Some(endpoint)
///     }
/// }
///
/// struct Consumer {
///     sender: Sender<u8>,
///     recver: Receiver<u8>,
/// }
///
/// impl Consumer {
///     fn new() -> Self {
///         let (sender, recver) = mpsc::channel();
///
///         Self {
///             sender,
///             recver,
///         }
///     }
/// }
///
/// impl Register for Consumer {
///     type Endpoint = Sender<u8>;
///
///     fn register(&mut self, other: &mut impl Connect<Self>) {
///         other.on_connection(self.sender.clone())
///     }
/// }
///
/// let mut consumer = Consumer::new();
/// let mut producer1 = Producer::default();
/// let mut producer2 = Producer::default();
///
/// consumer.register(&mut producer1);
/// consumer.register(&mut producer2);
/// ```
pub trait Register {
    /// The type used to communicate with some [`Connect`].
    type Endpoint;

    /// Connects the [`Register`] to the `other` entity with must implement [`Connect`] of `self`.
    ///
    /// This function should pass a `Endpoint` to `other` by calling the [`Connect::on_connection`] function.
    fn register(&mut self, other: &mut impl Connect<Self>);
}

/* ---------- */

/// A type implementing this trait can be connected to some [`Register`].
///
/// # Examples
///
/// ```
/// # use employees::*;
/// # use std::sync::mpsc::{self, Receiver, Sender};
/// #[derive(Default)]
/// struct Producer {
///     sender: Option<Sender<u8>>,
/// }
///
/// impl Connect<Consumer> for Producer {
///     fn on_connection(&mut self, endpoint: Sender<u8>) {
///         self.sender = Some(endpoint)
///     }
/// }
///
/// struct Consumer {
///     sender: Sender<u8>,
///     recver: Receiver<u8>,
/// }
///
/// impl Consumer {
///     fn new() -> Self {
///         let (sender, recver) = mpsc::channel();
///
///         Self {
///             sender,
///             recver,
///         }
///     }
/// }
///
/// impl Register for Consumer {
///     type Endpoint = Sender<u8>;
///
///     fn register(&mut self, other: &mut impl Connect<Self>) {
///         other.on_connection(self.sender.clone())
///     }
/// }
///
/// let mut consumer = Consumer::new();
/// let mut producer1 = Producer::default();
/// let mut producer2 = Producer::default();
///
/// consumer.register(&mut producer1);
/// consumer.register(&mut producer2);
/// ```
pub trait Connect<S: Register + ?Sized> {
    /// Sets the endpoint of the communication channel between `self` and `S`.
    fn on_connection(&mut self, endpoint: S::Endpoint);
}

/* ---------- */

#[cfg(test)]
mod tests {
    use super::*;

    use std::cell::RefCell;
    use std::rc::Rc;

    /* ---------- */

    #[derive(Debug, Default)]
    struct Foo {
        recver: Option<Rc<RefCell<u8>>>,
    }

    impl Foo {
        fn val(&mut self) -> Option<u8> {
            self.recver.as_ref().map(|inner| *inner.borrow())
        }
    }

    impl Connect<Bar> for Foo {
        fn on_connection(&mut self, channel: Rc<RefCell<u8>>) {
            let _ = self.recver.insert(channel);
        }
    }

    /* ---------- */

    #[derive(Debug, Default)]
    struct Bar {
        sender: Rc<RefCell<u8>>,
    }

    impl Bar {
        fn val(&self) -> u8 {
            *self.sender.borrow()
        }

        fn set_val(&mut self, val: u8) {
            *self.sender.borrow_mut() = val
        }
    }

    impl Register for Bar {
        type Endpoint = Rc<RefCell<u8>>;

        fn register(&mut self, client: &mut impl Connect<Self>) {
            client.on_connection(self.sender.clone())
        }
    }

    /* ---------- */

    #[test]
    fn reception() {
        let mut foo = Foo::default();
        let mut bar = Bar::default();

        assert!(
            foo.recver.is_none(),
            "foo shouldn't be connected to anything"
        );

        bar.register(&mut foo);
        assert!(foo.recver.is_some(), "foo be connected to bar");
        assert_eq!(foo.val(), Some(bar.val()));

        bar.set_val(8);
        assert_eq!(foo.val(), Some(bar.val()));
    }
}
