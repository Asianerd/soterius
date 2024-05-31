use std::collections::HashMap;
use std::fs;
use std::sync::Mutex;

use rand::prelude::*;

use rocket::http::Status;
use rocket::request::Request;
use rocket::data::{self, Data, FromData};
use rocket::outcome::Outcome;
use rocket::State;
use serde::{Deserialize, Serialize};

use crate::account_handler::AccountHandler;

const USER_ID_MAX: u128 = 4294967296u128; // 16^8, 2^32
const USER_ID_LENGTH: usize = 8usize; // number of letters for the code
// so that when represented in hex, itll be an 8 letter code

// XXXX-XXXX <- using this instead, 4 billion users

// XXXX-XXXX-XXXX
// or maybe XXX-XXX-XXX-XXX more readable? 
// TODO: revisit in future

#[derive(Clone, Serialize, Deserialize)]
pub struct User {
    pub username: String,

    pub id: u128,
    pub password: String
}
impl User {
    pub fn save(account_handler: &AccountHandler) {
        fs::write("../../data/users.json", serde_json::to_string_pretty(&(
            account_handler.users.clone()
                .iter()
                .map(
                    |(id, u)| (u.username.clone(), (id.clone(), u.password.clone()))
                )
                .collect::<HashMap<String, (u128, String)>>()
            )).unwrap()).unwrap();
    }

    pub fn load() -> HashMap<u128, User> {
        let result: HashMap<String, (u128, String)> = serde_json::from_str(fs::read_to_string("../../data/users.json").unwrap().as_str()).unwrap();
        result
            .into_iter()
            .map(|(k, v)| (v.0, User {
                username: k.clone(),
                id: v.0,
                password: v.1.clone()
            }))
            .collect::<HashMap<u128, User>>()
    }

    pub fn username_exists(account_handler: &AccountHandler, username: &String) -> bool {
        for (_, user) in &account_handler.users {
            if user.username == *username {
                return true;
            } 
        }
        false
    }

    pub fn generate_user_id(account_handler: &AccountHandler) -> u128 {
        let fallback = account_handler.users
            .keys()
            .max()
            .map_or(0, |i| i + 1);

        let mut rng = rand::thread_rng();
        for _ in 0..1000 {
            // try generate for 1k times, else, resort to fallback
            let candidate = rng.gen_range(0..USER_ID_MAX);
            if account_handler.users.contains_key(&candidate) {
                continue;
            }

            return candidate;
        }
        fallback
    }

    pub fn lookup_user_id(account_handler: &AccountHandler, username: &String) -> Option<u128> {
        if !User::username_exists(account_handler, username) {
            return None;
        }
        account_handler.users
            .iter()
            .filter(|(_, u)| u.username == *username)
            .map(|(id, _)| *id)
            .next()
    }
}


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LoginInformation {
    pub username: String,
    pub password: String
}
impl LoginInformation {
    pub fn login(&self, account_handler: &AccountHandler) -> LoginResult {
        match User::lookup_user_id(account_handler, &self.username) {
            Some(id) => if account_handler.users.get(&id).unwrap().password == self.password { LoginResult::Success(id) } else { LoginResult::PasswordWrong },
            None => LoginResult::UsernameNoExist
        }
    }

    pub fn signup(&self, account_handler: &mut AccountHandler) -> LoginResult {
        if User::username_exists(account_handler, &self.username) {
            return LoginResult::UsernameTaken;
        }

        let id = User::generate_user_id(account_handler);
        account_handler.users.insert(id, User {
            id,
            username: self.username.clone(),
            password: self.password.clone()
        });
        account_handler.save();

        LoginResult::Success(id)
    }
}

#[rocket::async_trait]
impl<'l> FromData<'l> for LoginInformation {
    type Error = LoginInfoParseError;

    async fn from_data(_req: &'l Request<'_>, mut data: Data<'l>) -> data::Outcome<'l, Self> {
        let result = data.peek(512).await.to_vec();

        if result.is_empty() {
            // return OutCome::Error
            return Outcome::Error((
                Status::Ok,
                LoginInfoParseError::Empty
            ))
        }
        
        let result = result.iter().map(|x| (x.clone()) as char).collect::<String>();
        let result: HashMap<String, String> = serde_json::from_str(result.as_str()).unwrap();

        Outcome::Success(LoginInformation {
            username: result.get("username").unwrap().clone(),
            password: result.get("password").unwrap().clone()
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum LoginInfoParseError {
    Success,

    ParsingError,

    Empty
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum LoginResult {
    Success(u128),
    UsernameNoExist,
    PasswordNoExist, // consistency issue

    PasswordWrong,

    UsernameTaken,
}


// #region api calls
#[post("/", data="<login>")]
pub fn login(db: &State<Mutex<AccountHandler>>, login: LoginInformation) -> String {
    let db = db.lock().unwrap();
    serde_json::to_string(&login.login(&db)).unwrap()
}

#[post("/", data="<login>")]
pub fn signup(db: &State<Mutex<AccountHandler>>, login: LoginInformation) -> String {
    let mut db = db.lock().unwrap();
    serde_json::to_string(&login.signup(&mut db)).unwrap()
}
