//! The PipeWire backend: connects to the daemon on its own thread, keeps a [`State`] of
//! devices, cards, streams, the clock and the graph, publishes it through a
//! [`Driver`](crate::model::Driver) and carries out [`Command`](crate::model::Command)s.
//! See `crate::model` for the contract.
//!
//! Not written yet: the backend agent fills this module in.

#[allow(unused_imports)]
use crate::model::{Audio, State};

/// Connects to PipeWire and starts the backend thread.
pub fn start() -> Result<Audio, String> {
    pipewire::init();
    Err("the PipeWire backend is not implemented yet; run with --mock".into())
}
