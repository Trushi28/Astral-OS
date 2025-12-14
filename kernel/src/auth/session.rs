//! Session Management
//! Tracks currently logged in user

use super::User;
use spin::Mutex;
use alloc::string::String;

pub struct Session {
    pub user: Option<User>,
    pub logged_in: bool,
}

impl Session {
    pub const fn new() -> Self {
        Self {
            user: None,
            logged_in: false,
        }
    }
    
    pub fn login(&mut self, user: User) {
        self.user = Some(user);
        self.logged_in = true;
    }
    
    pub fn logout(&mut self) {
        self.user = None;
        self.logged_in = false;
    }
    
    pub fn is_root(&self) -> bool {
        self.user.as_ref().map(|u| u.is_root()).unwrap_or(false)
    }
    
    pub fn username(&self) -> String {
        self.user.as_ref().map(|u| u.username.clone()).unwrap_or_else(|| String::from("guest"))
    }
}

pub static CURRENT_SESSION: Mutex<Session> = Mutex::new(Session::new());
