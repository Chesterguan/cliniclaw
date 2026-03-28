/// Library entry point for cliniclaw-api.
///
/// Exposes the router builder and AppState so integration tests can construct
/// an in-process app without binding a TCP port.
pub mod config;
pub mod error;
pub mod routes;
pub mod state;
