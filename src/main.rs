#![feature(proc_macro_hygiene, decl_macro, never_type)]
#[macro_use] extern crate rocket;
#[macro_use] extern crate rocket_contrib;
#[macro_use] extern crate validator_derive;
extern crate validator;

use rocket::config::ConfigError;
use rocket::fairing::AdHoc;
use rocket::request::{LenientForm, Form, FlashMessage};
use rocket::response::{Flash, Responder, Redirect, status};
use rocket::http::{Cookie, Cookies};
use validator::Validate;
use parity_scale_codec::{Encode, Decode};
use uuid::Uuid;

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::default::Default;
use sled;
use rocket::State;
use rocket_contrib::{
    templates::{Template, tera::Context},
    serve::StaticFiles,
};

mod model;
mod auth;
mod mail;

pub struct Database(sled::Db);
pub type Index = Vec<[u8; 16]>;

pub const IDX_OPEN_GRANTS: &'static str  = "grants_open";
pub const REL_GRANTS_PREFIX: &'static str  = "related_";

// FIXME: still no proper way to render-template-to-string for emails
//        replace when fixed https://github.com/SergioBenitez/Rocket/issues/1177
pub struct TemplateRenderer<'a,'r>(&'a rocket::Request<'r>);
impl <'_a, '_r> TemplateRenderer<'_a, '_r> {
    pub fn render<S,C>(&self, name: S, context: C) -> String
    where S: Into<std::borrow::Cow<'static, str>>, C: serde::Serialize, S: std::fmt::Display
    {
        use rocket_contrib::templates::Template;
        use rocket::response::Responder;
        let template = Template::render(name.into(), context);
        let mut response = template.respond_to(self.0).expect("Rendering can't fail.");
        response.body_string().unwrap_or_else(String::new)
    }
}
impl<'a, 'r> rocket::request::FromRequest<'a, 'r> for TemplateRenderer<'a,'r> {
    type Error = !;
    fn from_request(request: &'a rocket::Request<'r>) -> rocket::request::Outcome<Self, Self::Error> {
        rocket::request::Outcome::Success(TemplateRenderer(&request))
    }
}

#[get("/")]
fn index(flash: Option<FlashMessage>, user: Option<auth::User>) -> Template {
    let context = default_context(flash, user);
    Template::render("index", &context)
}


#[get("/login")]
fn already_logged_in(_user: auth::User) -> Redirect {
    Redirect::to(uri!(list))
}

#[get("/logout")]
fn logout(_user: auth::User, mut cookies: Cookies) -> Redirect {
    cookies.remove_private(Cookie::named("username"));
    Redirect::to(uri!(index))
}

#[get("/login", rank=2)]
fn login() -> Template {
    let mut context = Context::new();
    context.insert("form", &auth::LoginForm::default());
    context.insert("errors", "");
    Template::render("auth/login", &context)
}

#[post("/login",  data = "<login>")]
fn post_login(login: Form<auth::LoginForm>, db: State<auth::UserDatabase>, mut cookies: Cookies) -> Result<Redirect, Template> {
    if db.login(&login) {
        cookies.add_private(
            Cookie::build("username", login.username().clone())
            .http_only(true)
            .secure(true)
            .finish()
        );
        return Ok(Redirect::to(uri!(list)))
    } 
    
    let mut context = Context::new();
    let mut msg = HashMap::new();
    msg.insert("msg", "Login failed");
    msg.insert("name", "error");
    context.insert("flash_message", &msg);

    context.insert("form", &login.into_inner());
    context.insert("errors", "");
    Err(Template::render("auth/login", &context))
}

#[get("/event-grants/new")]
fn new_event_grant() -> Template {
    let mut context = Context::new();
    context.insert("form", &model::EventGrantForm::default());
    context.insert("errors", "");
    Template::render("grants/event_grant_form", &context)
}

fn render_error(e: String) -> Template {
    let mut context = Context::new();
    context.insert("error", &e);
    Template::render("generic/error", &context)
}

#[post("/event-grants/new", data = "<event>")]
fn new_event_grant_post(
    event: LenientForm<model::EventGrantForm>,
    mailer: State<mail::EmailSender>,
    templater: TemplateRenderer,
    database: State<Database>
)  -> Result<Flash<Redirect>, status::BadRequest<Template>> {
    let event = event.into_inner();
    match event.validate() {
        Ok(_) => {
            let db_item = model::Model::from(event);
            loop {
                let id = Uuid::new_v4();
                if database.0.transaction::<_, (), ()>(|db| {
                    if db.insert(id.as_bytes(), db_item.encode())?.is_some() {
                        return Err(sled::transaction::ConflictableTransactionError::Conflict);
                    }

                    let (id1, id2) = db_item.get_rel_ids();
                    for field in vec![
                        IDX_OPEN_GRANTS.as_bytes(),
                        format!("{}{}", REL_GRANTS_PREFIX, id1).as_bytes(),
                        format!("{}{}", REL_GRANTS_PREFIX, id2).as_bytes(),
                    ] {
                        let idx_ival = db.get(field)?.unwrap_or_default();
                        let mut idx = Index::decode(&mut idx_ival.as_ref()).unwrap_or_default();
                        idx.push(*id.as_bytes());
                        db.insert(field, idx.encode())?;
                    }

                    Ok(())
                }).is_err() {
                    // let's try again
                    continue
                }

                return database.0.flush()
                    .map_err(|e| e.to_string())
                    .map(|_| uri!(view_grant: id.to_string()))
                    .and_then(|uri| {
                        if let Some(addr) = db_item.get_addr_info() {
                            let mut context = Context::new();
                            context.insert("grant", &db_item);
                            context.insert("uri", &uri.to_string());
                            let html = templater.render("grants/emails/angenommen", context);
                            let subject = "Dein Radikal*Fund Antrag ist eingegangen".to_string();
                            mail::send_email(&mailer, addr, subject, html)
                                .map(|_| uri)
                        } else {
                            Ok(uri)
                        }
                    })
                    .map(|uri| {
                        Flash::success(
                            Redirect::to(uri),
                            "Dein Antrag ist eingegangen. Danke!"
                        )
                    })
                    .map_err(|e| status::BadRequest(Some(render_error(e))))
            }
        }
        Err(errors) => {
            let mut context =  Context::new();
            println!("{:#?} –> {:#?}", event, errors);
            context.insert("form", &event);
            context.insert("errors", &errors);

            Err(status::BadRequest(
                Some(
                    Template::render(
                        "grants/event_grant_form",
                        &context
                    )
                )
            ))
        }
    }
}

fn get_or_404(database: &Database, id: &[u8]) -> Result<model::Model, status::NotFound<Template>> {
    match database.0.get(id) {
        Ok(Some(m)) => model::Model::decode(&mut m.as_ref()).map_err(|e|
                status::NotFound(render_error(format!("Error decoding item: {:}", e)))),
        Ok(None) => Err(status::NotFound(render_error("Entry not found".to_string()))),
        Err(e) => Err(status::NotFound(render_error(e.to_string()))),
    }
}

fn default_context(flash: Option<FlashMessage>, user: Option<auth::User>) -> Context {
    let mut context = Context::new();
    if let Some(msg) = flash {
        let m: HashMap<&str, &str> = vec![
            ("name", msg.name()),
            ("msg", msg.msg())
        ].into_iter().collect();
        context.insert("flash_message", &m);
    }
    if let Some(user) = user {
        context.insert("current_user", &user.username());
    }  else {
        context.insert("current_user", &false);
    }

    context
}

#[get("/list")]
fn list(database: State<Database>, flash: Option<FlashMessage>, user: auth::User)
    -> Result<Template, status::BadRequest<Template>> //  FIXME: proper error code
{
    let mut context = default_context(flash, Some(user));

    let entries = database.0.get(IDX_OPEN_GRANTS.as_bytes())
        .map_err(|e| status::BadRequest(Some(render_error(e.to_string()))))?
        .and_then(|idx_ival| Index::decode(&mut idx_ival.as_ref()).ok())
        .unwrap_or_default()
        .iter()
        .filter_map(| uuid | Uuid::from_slice(uuid.as_ref()).ok())
        .filter_map(| uuid |
            database.0.get(uuid.as_bytes())
                .ok()
                .and_then(|val_o| val_o.map(|val| (uuid, val)))
        )
        .filter_map(| (u, val)|
            model::Model::decode(&mut val.as_ref())
                .map(|m| (u.to_string(), m))
                .ok()
        )
        .fold(HashMap::<&'static str, Vec<(String, model::Model)>>::new(), 
            |mut m, (uuid, model)| {
                if let Some(target) = model.state_name() {
                    match m.entry(target) {
                        Entry::Occupied(mut o) => { o.get_mut().push((uuid, model)); },
                        Entry::Vacant(v) => { v.insert(vec![(uuid, model)]); }
                    }
                }
                m
            }
        );
    context.insert("entries", &entries);
    Ok(Template::render("grants/list", &context))
}

#[post("/v/<id>?<next>", data = "<form>")]
fn update_grant(
    id: String,
    next: Option<String>,
    form: Form<model::NextStageForm>,
    db: State<Database>,
    user: auth::User,
    mailer: State<mail::EmailSender>,
    templater: TemplateRenderer,
)  -> Result<Flash<Redirect>, status::BadRequest<Template>> {
    let uuid = Uuid::parse_str(&id)
        .map_err(|e|status::BadRequest(Some(render_error(e.to_string()))))?;
    let mut grant =  get_or_404(&db, uuid.as_bytes())
        .map_err(|e| status::BadRequest(Some(e.0)))?;

    let notification = grant.next_stage(user.username().clone(), form.into_inner())
        .map_err(|e|status::BadRequest(Some(render_error(e.to_string()))))?;

    db.0.insert(uuid.as_bytes(), grant.encode())
        .and_then(|_| db.0.flush())
        .map_err(|e| status::BadRequest(Some(render_error(e.to_string()))))?;

    notification.and_then(|(subject, path)| {
        grant.get_addr_info().map(|addr| {
            let mut context = Context::new();
            context.insert("grant", &grant);
            context.insert("uuid", &uuid.to_string());
            let html = templater.render(path, context);
            mail::send_email(&mailer, addr, subject, html)
                .map(|_| ())
        })
    });

    let id = uuid.to_string();
    let redir = match next {
        Some(v) => Redirect::to(v),
        None => Redirect::to(uri!(view_grant: id))
    };

    Ok(Flash::success(redir, format!(
        "Antrag '{:}' ist jetzt '{:}'",
        grant.title().expect("We have a grant"),
        grant.state_name().expect("We are a grant")
    )))
}

#[get("/v/<id>")]
fn view_grant(id: String, database: State<Database>, flash: Option<FlashMessage>, user: Option<auth::User>)
    -> Result<Template, status::NotFound<Template>>
{
    let uuid = Uuid::parse_str(&id).map_err(|e|status::NotFound(render_error(e.to_string())))?;
    let grant = get_or_404(&database, uuid.as_bytes())?;
    let with_related = user.is_some();

    let mut context = default_context(flash, user);
    context.insert("uuid", &uuid.to_string());
    
    if with_related {
        let (id1, id2) = grant.get_rel_ids();
        let mut related = Vec::new();

        for field in vec![
            format!("{}{}", REL_GRANTS_PREFIX, id1).as_bytes(),
            format!("{}{}", REL_GRANTS_PREFIX, id2).as_bytes(),
        ] {
            let idx_ival = database.0.get(field)
                .map_err(|e|status::NotFound(render_error(e.to_string())))?.unwrap_or_default();
            related.extend_from_slice(&Index::decode(&mut idx_ival.as_ref()).unwrap_or_default()[..]);
        }
        
        related.dedup();
        context.insert("related", &related
            .iter()
            .filter_map(| uuid | Uuid::from_slice(uuid.as_ref()).ok())
            .filter_map(| ext | { if uuid == ext { None } else { Some(ext) } })
            .filter_map(| uuid |
                database.0.get(uuid.as_bytes())
                    .ok()
                    .and_then(|val_o| val_o.map(|val| (uuid, val)))
            )
            .filter_map(| (u, val)|
                model::Model::decode(&mut val.as_ref())
                    .map(|m| (u.to_string(), m))
                    .ok()
            )
            .collect::<Vec<_>>()
        )
    }

    match grant {
        model::Model::EventGrant(g) => {
            context.insert("grant", &g);
            Ok(Template::render("grants/view_event_grant", &context))
        }

        model::Model::AktivistiGrant(g) => {
            context.insert("grant", &g);
            Ok(Template::render("grants/view_aktivisti_grant", &context))
        }
        _ => Err(status::NotFound(render_error("Unknown item type".to_owned())))
    }
}

fn main() {
    setup(rocket::ignite()).launch();
}

fn setup(rocket: rocket::Rocket) -> rocket::Rocket {
    rocket
        .attach(Template::fairing())
        .attach(AdHoc::on_attach("Database Config", |rocket| {
            let db_dir = rocket.config()
                .get_str("database")
                .unwrap_or("sled")
                .to_string();
            
            let db = sled::open(db_dir)
                .expect("Opening sled database failed");
            Ok(rocket.manage(Database(db)))
        }))
        .attach(AdHoc::on_attach("Email Config", |rocket| {
            let mail = match rocket.config()
                .get_table("mail") {
                    Ok(t) => mail::make_lettre_transport(t),
                    _ =>  Ok(mail::EmailSender::default())
                }.expect("Email Setup failed");
            Ok(rocket.manage(mail))
        }))
        .attach(AdHoc::on_attach("Login Config", |rocket| {
            let db: HashMap<String, String> = rocket.config()
                .get_table("users")
                .map(|t| t
                        .into_iter()
                        .filter_map(|(k, v)| v.clone().try_into::<String>().ok().map(|r| (k.clone(), r)))
                        .collect())
                .unwrap_or_default();
            Ok(rocket.manage(auth::UserDatabase::new(db)))
        }))
        .mount("/pub", StaticFiles::from("static"))
        .mount("/", routes![
            index,
            already_logged_in,
            post_login,
            login,
            logout,

            list,
            view_grant,
            update_grant,
            new_event_grant_post,
            new_event_grant,
        ])
}

#[cfg(test)]
mod test {
    use super::setup;
    use url::form_urlencoded::Serializer;
    use tempfile::{tempdir, TempDir};
    use rocket::local::{Client, LocalResponse};
    use std::collections::{BTreeMap, HashMap};
    use rocket::http::{ContentType, Status};
    use rocket::config::{Config, Environment};
    use select::predicate::{Predicate, Name, Attr, Class};
    use select;

    type Params = Vec<(&'static str, &'static str)>;

    struct TestClient {
        client: Client,
        _dir: TempDir
    }

    impl TestClient {
        fn new() -> TestClient {
            let tmpd = tempdir().expect("Creating tempdir failed");
            let mut config = Config::build(Environment::Staging)
                .address("127.0.0.1")
                .port(700)
                .workers(1)
                .unwrap();

            let mut extras = HashMap::new();
            let mut users = BTreeMap::new();
            users.insert("admin".to_string(), "test".to_string());
            extras.insert("database".to_string(), tmpd.path().to_str().unwrap().into());
            extras.insert("users".to_string(), users.into());

            config.set_extras(extras);
            let client = Client::new(setup(rocket::custom(config))).expect("client setup works");

            TestClient {
                client,
                _dir: tmpd,
            }
            
        }
    }

    fn default_event_grand_fields() -> Params {
        vec![ // minimal fields
            // grant info
            ("grant_amount", "150"),
            ("grant_cost_breakdown", "Bahnfahrt"),

            // event info
            ("event_name", "radikal.jetzt workshop"),
            ("event_description", "Awesome Workshop, den wo es geht"),
            ("event_organiser", "radikal.jetzt"),
            ("event_why", "Weil geil."),

            // person 
            ("person_name", "ben"),
            ("person_about_me", "it's me, mario"),
            ("person_email", "ben@example.org"),

            // bank info
            ("bank_iban", "DE 1234 5678 42"),

            // required extra
            ("extra_accepted_privacy", "true"),
            ("extra_accepted_coc",  "true"),
        ]
    }

    fn with_field(params: &mut Params, field: &'static str, value: &'static str) {
        let mut found: Option<usize> = None;

        for (idx, val) in params.iter().enumerate() {
            if val.0 == field {
                found = Some(idx);
                break
            }
        }
        if let Some(idx) = found {
            params.remove(idx);
        }

        params.push((field, value))
    } 

    fn admin_client() -> TestClient {
        let tc = TestClient::new();
        {
            let req = tc.client
                .post("/login")
                .body("username=admin&password=test")
                .header(ContentType::Form);
            let response = req.dispatch();

            assert_eq!(response.status(), Status::SeeOther);
        }
        tc
    }

    fn items_count(client: &Client, path: Option<&str>) -> usize {
        let mut resp = client.get(path.unwrap_or("/list")).dispatch();

        assert_eq!(resp.status(), Status::Ok);
        let list = select::document::Document::from_read(resp.body().unwrap().into_inner()).unwrap();
        list.find(Class("grant-item").and(Name("li")).child(
                Name("a").and(Attr("data-grant-id", ())))
            )
        .into_selection()
        .len()
    }

    #[test]
    fn smoketest() {
        let tc = TestClient::new();
        let req = tc.client.get("/");
        let response = req.dispatch();

        assert_eq!(response.status(), Status::Ok);

        let resp = tc.client.post("/event-grants/new")
            .header(ContentType::Form)
            .body(Serializer::new(String::new())
                .extend_pairs(default_event_grand_fields().iter())
                .finish()
            )
            .dispatch();
        assert_eq!(resp.status(), Status::SeeOther);


        // works if unknown fields are submitted
        let mut bad_params = Serializer::new(String::new());
        bad_params.extend_pairs(default_event_grand_fields().iter());
        bad_params.append_pair("this_doesnt_exist", "false");
        
        let resp = tc.client.post("/event-grants/new")
            .header(ContentType::Form)
            .body(bad_params.finish())
            .dispatch();
        assert_eq!(resp.status(), Status::SeeOther);

    }

    #[test]
    fn required_event_grant_fields() {
        let tc = TestClient::new();
        let req = tc.client.get("/");
        let response = req.dispatch();

        assert_eq!(response.status(), Status::Ok);
        let params = default_event_grand_fields();
        for x in 0..params.len() {
            let mut local_params = params.clone();
            let item = local_params.remove(x); // dropping an item

            let resp = tc.client.post("/event-grants/new")
                .header(ContentType::Form)
                .body(Serializer::new(String::new())
                    .extend_pairs(local_params.iter())
                    .finish())
                .dispatch();
            assert_eq!(resp.status(), Status::BadRequest, "Worked despite {:?} missing", item);
        };

        // but all there works
        let resp = tc.client.post("/event-grants/new")
            .header(ContentType::Form)
            .body(Serializer::new(String::new())
                .extend_pairs(params.iter())
                .finish())
            .dispatch();
        assert_eq!(resp.status(), Status::SeeOther);
    }

    #[test]
    fn can_admin() {
        let tc = admin_client();
        let resp = tc.client.get("/list").dispatch();

        assert_eq!(resp.status(), Status::Ok);
    }

    #[test]
    fn related_grants() {
        let tc = admin_client();
        let resp = tc.client.get("/list").dispatch();

        assert_eq!(resp.status(), Status::Ok);

        let mut main_info = default_event_grand_fields();
        with_field(&mut main_info, "person_email", "test@example.org");
        with_field(&mut main_info, "bank_iban", "OG00 12345 6789");

        let resp = tc.client.post("/event-grants/new")
            .header(ContentType::Form)
            .body(Serializer::new(String::new())
                .extend_pairs(main_info.iter())
                .finish()
            )
            .dispatch();
        
        assert_eq!(resp.status(), Status::SeeOther);
        let location = resp.headers().get("Location").next().expect("Location must be present");

        assert_eq!(items_count(&tc.client, Some(location.clone())), 0); // none

        let mut grant_info = default_event_grand_fields();
        with_field(&mut grant_info, "person_email", "test@example.org");

        let resp = tc.client.post("/event-grants/new")
            .header(ContentType::Form)
            .body(Serializer::new(String::new())
                .extend_pairs(grant_info.iter())
                .finish()
            )
            .dispatch();
        
        assert_eq!(resp.status(), Status::SeeOther);

        assert_eq!(items_count(&tc.client, Some(location.clone())), 1); // same email

        let mut grant_info = default_event_grand_fields();
        with_field(&mut grant_info, "bank_iban", "OG00 12345 6789");

        let resp = tc.client.post("/event-grants/new")
            .header(ContentType::Form)
            .body(Serializer::new(String::new())
                .extend_pairs(grant_info.iter())
                .finish()
            )
            .dispatch();
        
        assert_eq!(resp.status(), Status::SeeOther);

        assert_eq!(items_count(&tc.client, Some(location.clone())), 2); // same iban

        let mut grant_info = default_event_grand_fields();
        with_field(&mut grant_info, "bank_iban", "OG00 12345 1234");

        let resp = tc.client.post("/event-grants/new")
            .header(ContentType::Form)
            .body(Serializer::new(String::new())
                .extend_pairs(grant_info.iter())
                .finish()
            )
            .dispatch();
        
        assert_eq!(resp.status(), Status::SeeOther);

        assert_eq!(items_count(&tc.client, Some(location.clone())), 2); // differnt iban

        let mut grant_info = default_event_grand_fields();
        with_field(&mut grant_info, "person_email", "test@example.com");

        let resp = tc.client.post("/event-grants/new")
            .header(ContentType::Form)
            .body(Serializer::new(String::new())
                .extend_pairs(grant_info.iter())
                .finish()
            )
            .dispatch();
        
        assert_eq!(resp.status(), Status::SeeOther);

        assert_eq!(items_count(&tc.client, Some(location.clone())), 2); // different email
    
    }

    #[test]
    fn regular_flow() {
        let tc = admin_client();
        let resp = tc.client.get("/list").dispatch();

        assert_eq!(resp.status(), Status::Ok);

        let resp = tc.client.post("/event-grants/new")
            .header(ContentType::Form)
            .body(Serializer::new(String::new())
                .extend_pairs(default_event_grand_fields().iter())
                .finish()
            )
            .dispatch();
        
        assert_eq!(resp.status(), Status::SeeOther);
        let location = resp.headers().get("Location").next().expect("Location must be present");

        let resp = tc.client.get(location).dispatch();
        assert_eq!(resp.status(), Status::Ok);
    }

    #[test]
    fn submits_are_lisited() {
        let tc = admin_client();
        let resp = tc.client.get("/list").dispatch();

        assert_eq!(resp.status(), Status::Ok);

        for x in 1..5 {
            // submitting one
            let resp = tc.client.post("/event-grants/new")
                .header(ContentType::Form)
                .body(Serializer::new(String::new())
                    .extend_pairs(default_event_grand_fields().iter())
                    .finish()
                )
                .dispatch();
            assert_eq!(resp.status(), Status::SeeOther);
            
            assert_eq!(items_count(&tc.client, None), x);
        }
    }
}
