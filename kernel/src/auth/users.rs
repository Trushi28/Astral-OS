//! User Database
//! Stores user accounts with password hashes

use alloc::string::String;
use alloc::vec::Vec;
use spin::Mutex;

/// User account
#[derive(Clone)]
pub struct User {
    pub uid: u32,
    pub username: String,
    password_hash: [u8; 32],
    pub groups: Vec<u32>,
    pub home_dir: String,
}

impl User {
    pub fn new(uid: u32, username: &str, password: &str) -> Self {
        Self {
            uid,
            username: String::from(username),
            password_hash: hash_password(password),
            groups: Vec::new(),
            home_dir: if uid == 0 {
                String::from("/root")
            } else {
                alloc::format!("/home/{}", username)
            },
        }
    }
    
    pub fn is_root(&self) -> bool {
        self.uid == 0
    }
    
    pub fn verify_password(&self, password: &str) -> bool {
        let hash = hash_password(password);
        self.password_hash == hash
    }
}

/// Simple password hashing (SHA-256 simplified)
fn hash_password(password: &str) -> [u8; 32] {
    let mut hash = [0u8; 32];
    let bytes = password.as_bytes();
    
    // Simple hash - XOR with rotation
    for (i, &byte) in bytes.iter().enumerate() {
        let idx = i % 32;
        hash[idx] ^= byte;
        hash[(idx + 7) % 32] = hash[(idx + 7) % 32].wrapping_add(byte);
        hash[(idx + 13) % 32] = hash[(idx + 13) % 32].rotate_left(3);
    }
    
    // Mix rounds
    for _ in 0..64 {
        for i in 0..32 {
            hash[i] = hash[i].wrapping_add(hash[(i + 1) % 32].rotate_left(5));
        }
    }
    
    hash
}

/// User database
pub struct UserDatabase {
    users: Vec<User>,
}

impl UserDatabase {
    pub const fn new() -> Self {
        Self { users: Vec::new() }
    }
    
    pub fn init(&mut self) {
        // Default users
        let mut root = User::new(0, "root", "astral");
        root.groups.push(0); // wheel group
        
        let mut user = User::new(1000, "user", "password");
        user.groups.push(1000); // users group
        
        self.users.push(root);
        self.users.push(user);
    }
    
    pub fn authenticate(&self, username: &str, password: &str) -> Option<User> {
        for user in &self.users {
            if user.username == username && user.verify_password(password) {
                return Some(user.clone());
            }
        }
        None
    }
    
    pub fn get_user(&self, username: &str) -> Option<&User> {
        self.users.iter().find(|u| u.username == username)
    }
}

pub static AUTH_DB: Mutex<UserDatabase> = Mutex::new(UserDatabase::new());

pub fn init() {
    AUTH_DB.lock().init();
}
