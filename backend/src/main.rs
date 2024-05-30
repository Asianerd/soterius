use std::sync::Mutex;

#[macro_use] extern crate rocket;

mod utils;
mod cors;

mod account_handler;

mod user;

#[get("/")]
pub fn index() -> String {
    "can you understand me?".to_string()
}

#[launch]
fn rocket() -> _ {
    rocket::custom(rocket::config::Config::figment().merge(("port", 8000)))
        .manage(Mutex::new(account_handler::AccountHandler::load()))
        .mount("/", routes![index])
        .mount("/save", routes![account_handler::save])
        .mount("/load", routes![account_handler::load])
        .mount("/generate_users", routes![account_handler::generate_users])
        .mount("/debug", routes![account_handler::debug])

        .mount("/login", routes![user::login])
        .mount("/signup", routes![user::signup])
        .mount("/lookup_username", routes![user::get_user_id])
        .mount("/query_users", routes![user::query_users])
        .mount("/get_code", routes![user::get_code])

        .attach(cors::CORS)
}