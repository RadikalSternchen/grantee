#![feature(proc_macro_hygiene, decl_macro, never_type)]
#[macro_use] extern crate rocket;
#[macro_use] extern crate rocket_contrib;
#[macro_use] extern crate validator_derive;
extern crate validator;

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
    Redirect::to("/list")
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
        return Ok(Redirect::to("/list"))
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

struct Hostname(String);

impl<'a, 'r> rocket::request::FromRequest<'a, 'r> for Hostname {
    type Error = ();

    fn from_request(request: &'a rocket::Request<'r>) -> rocket::request::Outcome<Self, Self::Error> {
        let host_name = request
            .headers()
            .get_one("Host")
            .map(|e| e.to_string())
            .unwrap_or_else(|| std::env::var("VIRTUAL_HOST").unwrap_or("localhost".to_string()));
        rocket::Outcome::Success(Hostname(host_name))
    }
}

#[post("/event-grants/new", data = "<event>")]
fn new_event_grant_post(
    event: LenientForm<model::EventGrantForm>,
    mailer: State<mail::EmailSender>,
    templater: TemplateRenderer,
    database: State<Database>,
    hostname: Hostname,
)  -> Result<Redirect, status::BadRequest<Template>> {
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
                    .and_then(|_| db_item.get_addr_info().ok_or("No valid Email Addr given".to_string()))
                    .and_then(|addr| db_item.email_token()
                        .map(|t| (addr, t)).ok_or("No valid Token found".to_string()))
                    .and_then(|(addr, token)| {
                        let confirm_uri = format!("http://{}{}", hostname.0, uri!(confirm_grant: id.to_string(), token));
                        let mut context = Context::new();
                        context.insert("grant", &db_item);
                        context.insert("uri", &confirm_uri.to_string());
                        let html = templater.render("grants/emails/eingang", context);
                        let subject = "Bitte Email Bestätigen".to_string();
                        mail::send_email(&mailer, addr, subject, html)
                            .map(|_| uri!(view_grant: id.to_string()))
                    })
                    .map(|uri| Redirect::to(uri))
                    .map_err(|e| status::BadRequest(Some(render_error(e))))
            }
        }
        Err(errors) => {
            let mut context =  Context::new();

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

fn get_or_404(database: &Database, id: &[u8])
    -> Result<model::Model, status::NotFound<Template>>
{
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

#[get("/list?<show>")]
fn list(
    database: State<Database>, 
    flash: Option<FlashMessage>,
    user: auth::User,
    show: Option<String>,
)
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

    let shown = show.unwrap_or_default();
    context.insert("show_pending", &shown.contains("pending"));
    context.insert("show_archived", &shown.contains("archived"));

    Ok(Template::render("grants/list", &context))
}

#[post("/v/<id>?<next>", data = "<form>")]
fn update_grant(
    id: String,
    form: Form<model::NextStageForm>,
    next: Option<String>,
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

#[get("/confirm/<id>/<token>")]
fn confirm_grant(
    id: String,
    token: String,
    database: State<Database>,
) -> Result<Flash<Redirect>, status::NotFound<Template>> {
    let uuid = Uuid::parse_str(&id).map_err(|e|status::NotFound(render_error(e.to_string())))?;
    let mut grant = get_or_404(&database, uuid.as_bytes())?;
    if grant.state_name() != Some("pending") {
        return Ok(Flash::new(Redirect::to(uri!(view_grant: id)), "none", "Email bereits bestätigt."))
    }

    if grant.email_token() == Some(token) {
        grant.next_stage(
            "_Antragstellerin_".to_string(),
            model::NextStageForm::new_simple("incoming")
        ).map_err(|e|status::NotFound(render_error(e.to_string())))?;

        database.0.insert(uuid.as_bytes(), grant.encode())
            .and_then(|_| database.0.flush())
            .map_err(|e| status::NotFound(render_error(e.to_string())))?;

        Ok(Flash::success(Redirect::to(uri!(view_grant: id)), "Email Adresse bestätigt"))
    } else {
        Err(status::NotFound(render_error("Token stimmt nicht überein".to_owned())))
    }
}

fn gen_quoata_state(db: &Database, model: &model::Model) -> Result<String, String> {
    let (non_white, non_man) = model.check_identities().unwrap_or_default();
    if non_white && non_man {
        return Ok("ok".to_string())
    }

    let idx = db.0.get(IDX_OPEN_GRANTS.as_bytes())
        .map_err(|e| e.to_string())?
        .and_then(|idx_ival| Index::decode(&mut idx_ival.as_ref()).ok())
        .unwrap_or_default();

    let mut total = 0u16;
    let mut total_non_white = 0u16;
    let mut total_non_man = 0u16;

    for (non_w, non_m) in idx.iter()
        .filter_map(| uuid |
            db.0.get(uuid.as_ref())
            .ok()
            .unwrap_or_default()
            .and_then(|ivec| model::Model::decode(&mut ivec.as_ref()).ok())
        )
        .filter_map(| mdl | {
            if mdl.quota_relevant() {
                mdl.check_identities()
            } else {
                None
            }
        })
    {
        total += 1;
        if non_w {
            total_non_white += 1
        }
        if non_m {
            total_non_man += 1
        }
    }

    println!("{} {} {}", total, total_non_man, total_non_white);

    if !non_white {
        if (total_non_white * 100 / (total + 1)) < 50 {
            return Ok("breaks_poc".to_string())
        }
    }

    if !non_man {
        if (total_non_man * 100 / (total + 1)) < 75 {
            return Ok("breaks_women".to_string())
        }
    }

    return Ok("ok".to_string())
}

#[get("/v/<id>")]
fn view_grant(
    id: String,
    database: State<Database>,
    flash: Option<FlashMessage>,
    user: Option<auth::User>
) -> Result<Template, status::NotFound<Template>> {
    let uuid = Uuid::parse_str(&id).map_err(|e|status::NotFound(render_error(e.to_string())))?;
    let grant = get_or_404(&database, uuid.as_bytes())?;
    let with_current_user = user.is_some();

    let mut context = default_context(flash, user);
    context.insert("uuid", &uuid.to_string());
    
    if with_current_user {
        // add quota info
        context.insert("quota_state", &gen_quoata_state(&database, &grant)
            .map_err(|e|status::NotFound(render_error(e)))?);
        // find related items
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
            confirm_grant,
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
    use rocket::local::Client;
    use std::collections::{BTreeMap, HashMap};
    use rocket::http::{ContentType, Status};
    use rocket::config::{Config, Environment};
    use select::predicate::{Predicate, Name, Attr, Class};
    use serde::{Serialize, Deserialize};
    use serde_json;
    use select;

    type Params = Vec<(&'static str, String)>;

    struct TestClient {
        client: Client,
        _tmp_root: TempDir,
        tmp_db: TempDir,
        tmp_mail: TempDir,
    }

    impl TestClient {
        fn new() -> TestClient {
            let tmpd = tempdir().expect("Creating tempdir failed");
            let tmp_db = TempDir::new_in(tmpd.path()).unwrap();
            let tmp_mail = TempDir::new_in(tmpd.path()).unwrap();

            let mut config = Config::build(Environment::Staging)
                .address("127.0.0.1")
                .port(7000)
                .workers(1)
                .unwrap();

            // User Database
            let mut users = BTreeMap::new();
            users.insert("admin".to_string(), "test".to_string());

            // E-Mail Setup
            let mut email = BTreeMap::new();
            email.insert("transport".to_string(), "file".to_string());
            email.insert("from".to_string(), "test@example.org".to_string());
            email.insert("path".to_string(), tmp_mail.path().to_str().unwrap().into());

            let mut extras = HashMap::new();
            extras.insert("database".to_string(), tmp_db.path().to_str().unwrap().into());
            extras.insert("users".to_string(), users.into());
            extras.insert("mail".to_string(), email.into());

            config.set_extras(extras);
            let client = Client::new(setup(rocket::custom(config))).expect("client setup works");

            TestClient {
                client,
                tmp_mail,
                tmp_db,
                _tmp_root: tmpd,
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
        ].iter().map(|v| (v.0, v.1.to_string())).collect()
    }

    fn with_field(params: &mut Params, name: &'static str, value: String) {
        params.retain(|v| v.0 != name);
        params.push((name, value));
    }

    #[derive(Serialize, Deserialize)]
    struct StoredEmail {
        envelope: lettre::Envelope,
        message_id: String,
        message: Vec<u8>,
    }

    impl StoredEmail { 
        fn into_message(self) -> String {
            String::from_utf8(self.message).expect("Messages parse")
        }
    }

    fn retrieve_email(tc: &TestClient, to_addr: &str) -> Option<StoredEmail> {
        let to_addr = lettre::EmailAddress::new(to_addr.to_string()).expect("Email Address invalid");
        for entry in std::fs::read_dir(tc.tmp_mail.path()).ok()?.filter_map(|f| f.ok()) {
            if entry.file_type().is_ok() {
                if let Ok(f) = std::fs::File::open(entry.path()) {
                    if let Ok(e) = serde_json::from_reader::<_, StoredEmail>(f) {
                        for addr in e.envelope.to().iter() {
                            if addr == &to_addr {
                                return Some(e)
                            }
                        }
                    }
                }
            }
        }
        None
    }

    fn confirm_grant_of(tc: &TestClient, host_addr: &str, to_addr: &str) -> String {
        let msg = retrieve_email(&tc, to_addr).expect("Email was sent").into_message();
        let mut url_found : Option<&str> = None;
        for line in msg.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("http") {
                url_found = Some(trimmed);
                break;
            }
        }

        let confirm_url = url_found.expect("No connfirmation url found");
        assert!(confirm_url.starts_with(host_addr), "URL malformatted");
        let resp = tc.client.get(&confirm_url[host_addr.len()..]).dispatch();
        assert_eq!(resp.status(), Status::SeeOther);
        resp.headers().get_one("Location").expect("Location must be present").to_string()
    }

    fn move_grant_to_stage(tc: &TestClient, path: &str, next: &str) {
        let resp = tc.client.post(path)
            .header(ContentType::Form)
            .body(format!("next={}", next))
            .dispatch();
        
        assert_eq!(resp.status(), Status::SeeOther);
        let location = resp.headers().get_one("Location").expect("Location must be present");
        assert_eq!(grant_status(&tc.client, location), next, "Switching to stage {} failed", next);
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

    fn grant_status(client: &Client, path: &str) -> String {
        let mut resp = client.get(path).dispatch();

        assert_eq!(resp.status(), Status::Ok);
        let list = select::document::Document::from_read(resp.body().unwrap().into_inner()).unwrap();
        list.find(Name("li").and(Attr("data-grant-state", ())))
            .into_selection()
            .first()
            .expect("Every Grant Page has the status field")
            .attr("data-grant-state")
            .expect("Only showed up because it had the field")
            .trim()
            .to_string()
    }

    fn grant_quota(client: &Client, path: &str) -> String {
        let mut resp = client.get(path).dispatch();

        assert_eq!(resp.status(), Status::Ok);
        let list = select::document::Document::from_read(resp.body().unwrap().into_inner()).unwrap();
        list.find(Attr("data-quota-state", ()))
            .into_selection()
            .first()
            .expect("All Grants have the quota state field")
            .attr("data-quota-state")
            .expect("Only showed up because it had the field")
            .trim()
            .to_string()
    }

    fn submit_event_grant<F>(tc: &TestClient, f: F) -> String 
        where F: Fn(&mut Params) -> ()
    {

        let mut fields = default_event_grand_fields();
        f(&mut fields);

        let resp = tc.client.post("/event-grants/new")
            .header(ContentType::Form)
            .header(rocket::http::Header::new("Host", "forms.radikal.org"))
            .body(Serializer::new(String::new())
                .extend_pairs(fields.iter())
                .finish()
            )
            .dispatch();
        assert_eq!(resp.status(), Status::SeeOther);
        let location = resp.headers().get_one("Location").expect("Location must be present");
        assert_eq!(grant_status(&tc.client, location), "pending");
        location.to_string()
    }

    fn gen_archive(client: &TestClient) {
        // all good, quote-wise
        for x in 0..4 {
            // submitting one
            let addr = format!("woc{}@example.org",  x);
            let location = submit_event_grant(client, |mut fields| {
                with_field(&mut fields, "person_email", addr.clone());
                with_field(&mut fields, "id_woc", "y".to_string());
            });
            move_grant_to_stage(client, &location, "accepted");
            move_grant_to_stage(client, &location, "paid");
        }

        for x in 0..3 {
            // submitting one
            let addr = format!("poc{}@example.org",  x);
            let location = submit_event_grant(client, |mut fields| {
                with_field(&mut fields, "person_email", addr.clone());
                with_field(&mut fields, "id_non_man", "y".to_string());
            });
            move_grant_to_stage(client, &location, "accepted");
            move_grant_to_stage(client, &location, "paid");
        }

        for x in 0..2 {
            // submitting one
            let addr = format!("regular{}@example.org",  x);
            let location = submit_event_grant(client, |mut fields| {
                with_field(&mut fields, "person_email", addr.clone());
            });
            move_grant_to_stage(client, &location, "accepted");
            move_grant_to_stage(client, &location, "paid");
        }
    }

    #[test]
    fn submitting_works() {
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
        let location = resp.headers().get_one("Location").expect("Location must be present");
        assert_eq!(grant_status(&tc.client, location), "pending");

        // works if unknown fields are submitted
        let mut bad_params = Serializer::new(String::new());
        bad_params.extend_pairs(default_event_grand_fields().iter());
        bad_params.append_pair("this_doesnt_exist", "false");
        
        let resp = tc.client.post("/event-grants/new")
            .header(ContentType::Form)
            .body(bad_params.finish())
            .dispatch();
        assert_eq!(resp.status(), Status::SeeOther);
        let location = resp.headers().get_one("Location").expect("Location must be present");
        assert_eq!(grant_status(&tc.client, location), "pending");
    }

    #[test]
    fn submit_email_flow_works() {
        let tc = TestClient::new();
        let req = tc.client.get("/");
        let response = req.dispatch();

        assert_eq!(response.status(), Status::Ok);

        let resp = tc.client.post("/event-grants/new")
            .header(ContentType::Form)
            .header(rocket::http::Header::new("Host", "test.radikal.org"))
            .body(Serializer::new(String::new())
                .extend_pairs(default_event_grand_fields().iter())
                .finish()
            )
            .dispatch();
        assert_eq!(resp.status(), Status::SeeOther);
        let location = resp.headers().get("Location").next().expect("Location must be present");
        assert_eq!(grant_status(&tc.client, location), "pending");

        let new_location = confirm_grant_of(&tc, "http://test.radikal.org", "ben@example.org");

        assert_eq!(location, new_location, "Locations differ");

        // the grant is now confirmed
        assert_eq!(grant_status(&tc.client, location), "incoming");
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
        with_field(&mut main_info, "person_email", "test@example.org".into());
        with_field(&mut main_info, "bank_iban", "OG00 12345 6789".into());

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
        with_field(&mut grant_info, "person_email", "test@example.org".into());

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
        with_field(&mut grant_info, "bank_iban", "OG00 12345 6789".into());

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
        with_field(&mut grant_info, "bank_iban", "OG00 12345 1234".into());

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
        with_field(&mut grant_info, "person_email", "test@example.com".into());

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
            .header(rocket::http::Header::new("Host", "community.radikal.org"))
            .body(Serializer::new(String::new())
                .extend_pairs(default_event_grand_fields().iter())
                .finish()
            )
            .dispatch();
        
        assert_eq!(resp.status(), Status::SeeOther);
        let location = resp.headers().get("Location").next().expect("Location must be present");

        let resp = tc.client.get(location).dispatch();
        assert_eq!(resp.status(), Status::Ok);

        confirm_grant_of(&tc, "http://community.radikal.org", "ben@example.org");
        assert_eq!(grant_status(&tc.client, location), "incoming");
        move_grant_to_stage(&tc, location, "checking");
        move_grant_to_stage(&tc, location, "board");
        move_grant_to_stage(&tc, location, "accepted");
        move_grant_to_stage(&tc, location, "paid");
    }

    #[test]
    fn submits_are_listed() {
        let tc = admin_client();
        let resp = tc.client.get("/list").dispatch();

        assert_eq!(resp.status(), Status::Ok);

        for x in 1..5 {
            // submitting one
            let addr = format!("addre{}@example.org",  x);
            let mut fields = default_event_grand_fields();
            with_field(&mut fields, "person_email", addr.clone());
            let resp = tc.client.post("/event-grants/new")
                .header(ContentType::Form)
                .header(rocket::http::Header::new("Host", "forms.radikal.org"))
                .body(Serializer::new(String::new())
                    .extend_pairs(fields.iter())
                    .finish()
                )
                .dispatch();
            assert_eq!(resp.status(), Status::SeeOther);
            let location = resp.headers().get_one("Location").expect("Location must be present");
            assert_eq!(grant_status(&tc.client, location), "pending");
            
            // not shown when pending
            assert_eq!(items_count(&tc.client, None), x - 1);
            // but shown if enabled
            assert_eq!(items_count(&tc.client, Some("/list?show=pending")), x);

            confirm_grant_of(&tc, "http://forms.radikal.org", &addr);
            assert_eq!(grant_status(&tc.client, location), "incoming");

            // shown when incoming
            assert_eq!(items_count(&tc.client, None), x);
        }
    }

    #[test]
    fn quota_blocks() {
        let tc = admin_client();
        let resp = tc.client.get("/list").dispatch();

        assert_eq!(resp.status(), Status::Ok);
        gen_archive(&tc);

        let location = submit_event_grant(&tc, |_| {});
        assert_eq!(grant_status(&tc.client, &location), "pending");
        assert_eq!(&grant_quota(&tc.client, &location), "breaks_poc");
    }

    #[test]
    fn quota_blocks_bipoc() {
        let tc = admin_client();
        let resp = tc.client.get("/list").dispatch();

        assert_eq!(resp.status(), Status::Ok);
        gen_archive(&tc);

        let location = submit_event_grant(&tc, |mut fields| {
            with_field(&mut fields, "id_bipoc", "y".to_string());
        });
        assert_eq!(grant_status(&tc.client, &location), "pending");
        assert_eq!(&grant_quota(&tc.client, &location), "breaks_women");
    }

    #[test]
    fn quota_accepts_woc() {
        let tc = admin_client();
        let resp = tc.client.get("/list").dispatch();

        assert_eq!(resp.status(), Status::Ok);
        gen_archive(&tc);

        let location = submit_event_grant(&tc, |mut fields| {
            with_field(&mut fields, "id_woc", "y".to_string());
        });
        assert_eq!(grant_status(&tc.client, &location), "pending");
        assert_eq!(&grant_quota(&tc.client, &location), "ok");
    }

    #[test]
    fn quota_accepts_non_man_poc() {
        let tc = admin_client();
        let resp = tc.client.get("/list").dispatch();

        assert_eq!(resp.status(), Status::Ok);
        gen_archive(&tc);

        let location = submit_event_grant(&tc, |mut fields| {
            with_field(&mut fields, "id_bipoc", "y".to_string());
            with_field(&mut fields, "id_agender", "y".to_string());
        });
        assert_eq!(grant_status(&tc.client, &location), "pending");
        assert_eq!(&grant_quota(&tc.client, &location), "ok");
    }

    #[test]
    fn quota_accepts_non_man_muslima() {
        let tc = admin_client();
        let resp = tc.client.get("/list").dispatch();

        assert_eq!(resp.status(), Status::Ok);
        gen_archive(&tc);

        let location = submit_event_grant(&tc, |mut fields| {
            with_field(&mut fields, "id_muslima", "y".to_string());
            with_field(&mut fields, "id_non_man", "y".to_string());
        });
        assert_eq!(grant_status(&tc.client, &location), "pending");
        assert_eq!(&grant_quota(&tc.client, &location), "ok");
    }

}
