//! Player aggregate library.

pub mod handler;
pub mod handlers;
pub mod state;

pub use handler::PlayerHandler;
pub use handlers::*;
pub use state::{PlayerState, STATE_ROUTER};
