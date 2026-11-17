//! Milestone 2 §3: deterministic fault injection for recovery tests.
//!
//! A session has no injector unless a test installs one with [`crate::Session::inject_faults`],
//! so production runs only pay for one `None` check per point. A test sees every checkpoint
//! save (before and after the atomic write) and every named step that writes files, and decides
//! what happens there:
//! - [`Fault::Fail`] is an ordinary error, such as a full disk. Oxidize's normal error handling
//!   runs: the attempt is rejected and that rejection is saved, if saving still works.
//! - [`Fault::Crash`] models the process dying. It unwinds with [`InjectedCrash`], and the session
//!   skips all recovery writes, so the disk holds exactly what a killed process would leave. The
//!   test then drops the session and reopens it.
//! - [`Fault::Torn`] (save points only) is a crash midway through writing the new checkpoint:
//!   half of it lands in the temporary file and `state.json` is never replaced.
use std::cell::{Cell, RefCell};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FaultPoint {
    /// Checkpoint save number `n` (counting from 1 for this session), before anything is written.
    BeforeSave(usize),
    /// Checkpoint save number `n`, after `state.json` was replaced.
    AfterSave(usize),
    /// A named step that writes files outside `state.json`.
    Step(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    None,
    Fail,
    Crash,
    Torn,
}

/// The error a [`Fault::Crash`] or [`Fault::Torn`] unwinds with. Recovery code never handles it.
#[derive(Debug)]
pub struct InjectedCrash(pub FaultPoint);

impl std::fmt::Display for InjectedCrash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "injected crash at {:?}", self.0)
    }
}
impl std::error::Error for InjectedCrash {}

/// The error a [`Fault::Fail`] returns.
#[derive(Debug)]
pub struct InjectedFailure(pub FaultPoint);

impl std::fmt::Display for InjectedFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "injected write failure at {:?}", self.0)
    }
}
impl std::error::Error for InjectedFailure {}

pub type Injector = Box<dyn FnMut(&FaultPoint) -> Fault>;

#[derive(Default)]
pub(crate) struct Faults {
    injector: RefCell<Option<Injector>>,
    saves: Cell<usize>,
}

impl Faults {
    pub(crate) fn install(&self, injector: Injector) {
        *self.injector.borrow_mut() = Some(injector);
    }
    pub(crate) fn next_save(&self) -> usize {
        self.saves.set(self.saves.get() + 1);
        self.saves.get()
    }
    pub(crate) fn decide(&self, point: &FaultPoint) -> Fault {
        self.injector
            .borrow_mut()
            .as_mut()
            .map_or(Fault::None, |inject| inject(point))
    }
    /// Turn the decision at `point` into an error. Use where a torn write means nothing.
    pub(crate) fn check(&self, point: FaultPoint) -> anyhow::Result<()> {
        match self.decide(&point) {
            Fault::None => Ok(()),
            Fault::Fail => Err(InjectedFailure(point).into()),
            Fault::Crash | Fault::Torn => Err(InjectedCrash(point).into()),
        }
    }
}

/// Whether `error` is a simulated process death, which recovery code must not handle.
pub(crate) fn is_crash(error: &anyhow::Error) -> bool {
    error.chain().any(|e| e.is::<InjectedCrash>())
}
