use rocket::{
    request::{Request, Outcome, FromForm, FromRequest},
    outcome::IntoOutcome, 
};
use std::collections::HashMap;
use serde::{Serialize};

#[derive(Debug, Default, Serialize, FromForm)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

impl LoginForm {
    pub fn username(&self) -> &String {
        &self.username
    }
}


pub struct UserDatabase(HashMap<String, String>);

impl UserDatabase {
    pub fn new(h: HashMap<String, String>) -> Self {
        UserDatabase(h)
    }
    pub fn login(&self, login: &LoginForm) -> bool {
        if let Some(stored) = self.0.get(&login.username) {
            *stored == login.password
        } else {
            false
        }
    }
}
pub struct User(String);

impl User {
    pub fn username(&self) -> &String {
        &self.0
    }
}

impl<'a, 'r> FromRequest<'a, 'r> for User {
    type Error = ();

    fn from_request(request: &'a Request<'r>) -> Outcome<User, ()> {
        request.cookies()
            .get_private("username")
            .and_then(|cookie| cookie.value().parse().ok())
            .map(|id| User(id))
            .or_forward(())
    }
}
