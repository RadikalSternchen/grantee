#![feature(proc_macro_hygiene, decl_macro)]

use std::collections::HashMap;
use rocket_contrib::templates::Template;
use rocket_contrib::serve::StaticFiles;

#[macro_use] extern crate rocket;

#[get("/")]
fn index() -> Template {
    let context = HashMap::<String, String>::new();
    Template::render("example", &context)
}
fn main() {
    rocket::ignite()
        .attach(Template::fairing())
        .mount("/pub", StaticFiles::from("static"))
        .mount("/", routes![index])
        .launch();
}
