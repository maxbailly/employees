/// A trait that is implemented on type that act as some kind
/// of server in a multi-threaded context.
pub trait Register {
    /// The type used to communitate with the [`Connect`].
    type Endpoint;

    /// Connects the [`Register`] to the `other` entity. The latter
    /// must be a [`Connect`] of `self`.
    ///
    /// This method should pass a `Endpoint` to the given entity.
    fn register(&mut self, other: &mut impl Connect<Self>);
}

/* ---------- */

/// Connects two types in a server/client relationship.
///
/// A type implementing this trait can be connected to some [`Register`]
/// to send or receive messages using the `endpoint`.
pub trait Connect<S: Register + ?Sized> {
    /// Sets the endpoint of the implementor.
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
