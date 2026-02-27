//! Table aggregate library.

pub mod handler;
pub mod handlers;
pub mod state;

pub use handler::TableHandler;
pub use handlers::*;
pub use state::{rebuild_state, SeatState, TableState, STATE_ROUTER};
