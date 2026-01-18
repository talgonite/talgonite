// Lightweight re-export of the pre-login session without pulling the old session plugin
// This points at the existing implementation in src/session/pre_login.rs

#[path = "session/pre_login.rs"]
mod pre_login_impl;

pub use pre_login_impl::LoginError;
pub use pre_login_impl::PreLoginSession;
