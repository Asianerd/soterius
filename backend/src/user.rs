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

use crate::{account_handler::AccountHandler, utils};

const USER_ID_MAX: u128 = 4294967296u128; // 16^8, 2^32
const USER_ID_LENGTH: usize = 8usize; // number of letters for the code
// so that when represented in hex, itll be an 8 letter code

// XXXX-XXXX <- using this instead, 4 billion users

// XXXX-XXXX-XXXX
// or maybe XXX-XXX-XXX-XXX more readable? 
// TODO: revisit in future

#[derive(Clone, Serialize, Deserialize)]
pub struct User {
    pub id: u128,
    pub username: String
}
impl User {
    pub fn save(account_handler: &AccountHandler) {
        fs::write("data/users.json", serde_json::to_string_pretty(&account_handler.users).unwrap()).unwrap();
    }

    pub fn load() -> HashMap<u128, User> {
        serde_json::from_str(fs::read_to_string("data/users.json").unwrap().as_str()).unwrap()
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

    // #region user IDs
    pub fn encode_id(user_id: &u128) -> String {
        format!("{:0>8}", format!("{:X}", user_id).to_lowercase())
    }

    pub fn decode_id(code: &String) -> Option<u128> {
        if !User::is_valid_code(&code) {
            return None;
        }

        let code = User::sanitize_code(&code);
        Some(i64::from_str_radix(code.as_str(), 16).unwrap() as u128)
    }

    pub fn sanitize_code(code: &String) -> String {
        code.replace("#", "").replace("-", "")
        // #1234-1234 -> 12341234
    }

    pub fn is_valid_code(code: &String) -> bool {
        let code = User::sanitize_code(&code);
        // in case they give in #XXXX-XXXX
        if code.len() != USER_ID_LENGTH {
            return false;
        }
        // is able to be converted back to u128
        i64::from_str_radix(&code.as_str(), 16).is_ok()
    }
    // #endregion

    // #region querying
    pub fn query_username(account_handler: &AccountHandler, username: String) -> Vec<(u128, String, String)> { // id, 8 letter code, username
        // returns vector of users containing the username/id
        let mut result: Vec<(u128, String, String)> = vec![];
        for u in account_handler.users.values() {
            if result.len() >= 50 {
                // show only 20 results
                break;
            }
            if u.username.to_lowercase().contains(&(username.to_lowercase())) {
                result.push((
                    u.id.clone(),
                    User::encode_id(&u.id.clone()),
                    u.username.clone()
                ));
            }
        }

        if User::is_valid_code(&username) {
            let user_id = User::decode_id(&username).unwrap();

            if !account_handler.users.contains_key(&user_id) {
                return result;
            }

            if !result.iter().map(|(i, _, _)| i.clone()).any(|i| i == user_id) {
                // does not contain the one with correct ID
                result.insert(0, (
                    user_id,
                    User::encode_id(&user_id),
                    account_handler.users.get(&user_id).unwrap().username.clone()
                ));
            } else {
                // bring the matching id to the top
                for (index, u) in result.iter().enumerate() {
                    if u.0 == user_id {
                        let a = result.remove(index);
                        result.insert(0, a);
                        break;
                    }
                }
            }
        }

        result
    }
    // #endregion
}


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LoginInformation {
    pub username: String,
    pub password: String
}
impl LoginInformation {
    // handles anything to do with password or logging in
    fn get_passwords() -> HashMap<u128, String> {
        serde_json::from_str(fs::read_to_string("data/passwords.json").unwrap().as_str()).unwrap()
    }

    fn add_password(user_id: u128, password: &String) {
        let mut p = LoginInformation::get_passwords();
        p.insert(user_id, password.clone());
        fs::write("data/passwords.json", serde_json::to_string_pretty(&p).unwrap()).unwrap();
    }

    pub fn login(&self, account_handler: &AccountHandler) -> LoginResult {
        match User::lookup_user_id(account_handler, &self.username) {
            Some(id) => match LoginInformation::get_passwords().get(&id) {
                Some(p) => if self.password == *p { LoginResult::Success(id) } else { LoginResult::PasswordWrong },
                None => LoginResult::PasswordNoExist
            },
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
            username: self.username.clone()
        });
        LoginInformation::add_password(id, &self.password);
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

// apply caching
// remove username from cache when username is changed
#[get("/<username>")]
pub fn get_user_id(db: &State<Mutex<AccountHandler>>, username: String) -> String {
    let db = db.lock().unwrap();
    let result = User::lookup_user_id(&db, &username);
    match result {
        Some(id) => utils::parse_response_to_string(Ok(id)),
        None => utils::parse_response_to_string(Err(result))
    }
}

#[post("/", data="<login>")]
pub fn get_code(db: &State<Mutex<AccountHandler>>, login: LoginInformation) -> String {
    let db = db.lock().unwrap();
    let result = login.login(&db);
    match result {
        LoginResult::Success(user_id) => utils::parse_response_to_string(Ok(User::encode_id(&user_id))),
        _ => utils::parse_response_to_string(Err(result))
    }
}

#[post("/<query_string>", data="<login>")]
pub fn query_users(db: &State<Mutex<AccountHandler>>, login: LoginInformation, query_string: String) -> String {
    let db = db.lock().unwrap();
    let result = login.login(&db);
    match result {
        LoginResult::Success(_) => utils::parse_response_to_string(Ok(User::query_username(&db, urlencoding::decode(&query_string).unwrap().to_string()))),
        _ => utils::parse_response_to_string(Err(result))
    }
}
// #endregion
