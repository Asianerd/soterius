use std::{collections::HashMap, sync::Mutex};

use rocket::State;
use serde::{Deserialize, Serialize};

use crate::{user::{self, User}, utils};

#[derive(Clone, Serialize, Deserialize)]
pub struct AccountHandler {
    pub users: HashMap<u128, User>,
}
impl AccountHandler {
    pub fn new() -> AccountHandler {
        AccountHandler {
            users: HashMap::new()
        }
    }

    pub fn save(&self) {
        User::save(&self);
    }

    pub fn load() -> AccountHandler {
        let mut r = AccountHandler::new();
        r.users = User::load();

        r
    }
}

// #region api calls
#[get("/")]
pub fn load(db: &State<Mutex<AccountHandler>>) -> String {
    let mut db = db.lock().unwrap();
    *db = AccountHandler::load();
    "success".to_string()
}

#[get("/")]
pub fn save(db: &State<Mutex<AccountHandler>>) -> String {
    let db = db.lock().unwrap();
    db.save();
    "success".to_string()
}

#[get("/<number>")]
pub fn generate_users(db: &State<Mutex<AccountHandler>>, number: usize) -> String {
    let mut db = db.lock().unwrap();
    for _ in 0..number {
        let id = user::User::generate_user_id(&db);
        db.users.insert(id, user::User {
            id,
            username: utils::generate_name(&mut rand::thread_rng()),
            password: "password".to_string()
        });
    }
    "success".to_string()
}

#[get("/")]
pub fn debug(db: &State<Mutex<AccountHandler>>) -> String {
    let db = db.lock().unwrap();
    serde_json::to_string_pretty(&db.users).unwrap()
}
// #endregion
