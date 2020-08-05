
use parity_scale_codec::{Encode, Decode};
use rocket::request::{Form, FromForm, FormItems};
use serde::{Serialize, Deserialize};
use std::time::SystemTime;
use rand::thread_rng;
use rand::seq::SliceRandom;
use blake2::{Blake2b, Digest};

// A trait that the Validate derive will impl
use validator::{Validate, ValidationError};

static ICONS : [&str; 440] = ["👿","👹","👺","🤡","💩","👻","💀","👽","👾","🤖","🎃",
"🧳","🌂","🧵","🧶","👓","🕶","🥽","🥼","🦺","👔","👕","👖","🧣","🧤","🧥","🧦",
"👗","👘","🥻","🩱","🩲","🩳","👙","👚","👛","👜","👝","🎒","👞","👟","🥾","🥿","👠",
"👡","🩰","👢","👑","👒","🎩","🎓","🧢","⛑","💍","💼","🐶","🐹","🐰","🦊","🐻","🐼",
"🐨","🐯","🦁","🐮","🐷","🐽","🐸","🐵","🙈","🙉","🙊","🐒","🐔","🐧","🐦","🐤","🐣",
"🐥","🦆","🦅","🦉","🦇","🐺","🐗","🐴","🦄","🐝","🐛","🦋","🐌","🐞","🐜","🦟","🦗",
"🕷","🕸","🦂","🐢","🐍","🦎","🦖","🦕","🐙","🦑","🦐","🦞","🦀","🐡","🐠","🐟","🐬",
"🐳","🐋","🦈","🐊","🐅","🐆","🦓","🦍","🦧","🐘","🦛","🦏","🐪","🐫","🦒","🦘","🐃",
"🐂","🐄","🐎","🐖","🐏","🐑","🦙","🐐","🐕‍","🐈","🐓","🦃","🦚",
"🦜","🦢","🦩","🕊","🐇","🦝","🦨","🦡","🦦","🦥","🐁","🐀","🐿","🦔","🐾","🐉","🐲",
"🌵","🎄","🌲","🌳","🌴","🌱","🌿","☘","🍀","🎍","🎋","🍃","🍂","🍁","🍄","🐚","🌾",
"💐","🌷","🌹","🥀","🌺","🌸","🌼","🌻","🌞","🌝","💫","⭐️","🌟","✨","💥",
"🔥","🌈","💨","💧","💦","🌊","🍏","🍎","🍐","🍊","🍋","🍌","🍉","🍇","🍓","🍈","🍒",
"🍑","🥭","🍍","🥥","🥝","🍅","🍆","🥑","🥦","🥬","🥒","🌶","🌽","🥕","🧄","🧅","🥔",
"🍠","🥐","🥯","🍞","🥖","🥨","🧀","🥚","🍳","🧈","🥞","🧇","🥓","🥩","🍗","🍖","🦴",
"🌭","🍔","🍟","🍕","🥪","🥙","🧆","🌮","🌯","🥗","🥘","🥫","🍝","🍜","🍲","🍛","🍣",
"🍱","🥟","🦪","🍤","🍙","🍚","🍘","🍥","🥠","🥮","🍢","🍡","🍧","🍨","🍦","🥧","🧁",
"🍰","🎂","🍮","🍭","🍬","🍫","🍿","🍩","🍪","🌰","🥜","🍼","☕️","🍵","🧃",
"🥤","🍶","🍺","🍻","🥂","🍷","🥃","🍹","🧉","🍾","🧊","🥄","🍴","🥣","🥡","🥢","🧂",
"⚽️","🏀","🏈","⚾️","🥎","🎾","🏐","🏉","🥏","🎱","🪀","🏓","🏸","🏒","🏑","🥍","🏏",
"🥅","⛳️","🪁","🏹","🎣","🤿","🥊","🥋","🎽","🛹","🛷","🥌","🎭","🩰","🎨","🎤","🎼",
"🎹","🥁","🎷","🎺","🎸","🪕","🎻","🎲","♟","🎯","📱","📲","💽","💾","💿","📀","📼"
,"📸","🎥","🎞","📞","📠","⏰","📡","🔋","🔌","💡","🔦","🪔","🧯","💎","🧰","🔧",
"🔨","🔩","🧱","🧲","🔫","💣","🧨","🪓","🔪","🗡","⚔️","🛡","🚬","⚰️","⚱️","🏺","🔮",
"📿","🧿","🔭","🔬","🕳","🩹","🩺","💊","💉","🩸","🧬","🦠","🧫","🧪","🧹",
"🧺","🧻","🚽","🚰","🚿","🛁","🛀","🧼","🪒","🧽","🧴","🛎","🔑","🗝","🚪","🪑","🛋",
"🛏","🛌","🧸","🖼","🛍","🛒","🎁","🎈","🎏","🎀","🎊","🎉","🎎","🏮","🎐","🧧"];

fn make_random_title(len: u8) -> String {
    let mut rng = thread_rng();
    let mut s = String::new();
    for _ in 0..len {
        s.push_str(ICONS.choose(&mut rng).expect("Exists"))
    }
    s
} 

#[derive(Debug, Clone, Encode, Serialize, Deserialize, Decode)]
pub enum Identity {
    WoC,
    BIPoC,
    SintiRoma,
    Muslima,
    Jewish,
    NonWhite,
    Trans,
    Woman,
    Mother,
    NonMan,
    WithDisability,
    Inter,
    Agender,
}

impl Identity {
    fn from(inp: &str) -> Result<Identity, String> {
        Ok(match inp {
            "woc" => Self::WoC,
            "bipoc" => Self::BIPoC,
            "sinti_roma" => Self::SintiRoma,
            "muslima" => Self::Muslima,
            "jewish" => Self::Jewish,
            "non_white" => Self::NonWhite,
            "trans" => Self::Trans,
            "woman" => Self::Woman,
            "mother" => Self::Mother,
            "non_man" => Self::NonMan,
            "with_disability" => Self::WithDisability,
            "inter" => Self::Inter,
            "agender" => Self::Agender,
            i => return Err(format!("Unknown key {:}", i)),
        })
    }
}


#[derive(FromForm, Clone, Debug, Validate, Serialize, Deserialize, Encode, Default, Decode)]
pub struct PersonalDetails {
    #[validate(length(min = 1))]
    name: String,
    #[validate(length(min = 5))]
    about_me: String,
    online_personas: Option<String>,
    // contact
    #[validate(email)]
    email: String,
}

impl PersonalDetails {
    fn set(&mut self, key: &str, value: String) -> Result<(), String> {
        Ok(match key {
            "name" => { self.name = value; },
            "online_personas" => { self.online_personas = Some(value); },
            "email" => { self.email = value; },
            "about_me" => { self.about_me = value; },
            _ => {
                return Err(format!("Unknown key {:} on personal details", key));
            }
        })
    }
}

#[derive(Debug, Clone, Encode, Decode, Serialize, Deserialize)]
enum Person {
    Anonymised(String), // storing the hash of the E-Mail
    Detail(PersonalDetails), // FullInfo
}

impl Person {
    pub fn get_addr_info(&self) -> Option<(String, String)> {
        match self {
            Person::Detail(d) => Some((d.email.clone(), d.name.clone())),
            Person::Anonymised(_) => None,
        }
    }

    pub fn id(&self) -> String {
        match self {
            Person::Detail(d) => {
                format!("{:x}", Blake2b::new()
                    .chain(d.email.replace(" ", "").to_lowercase().as_bytes())
                    .finalize()
                )
            }
            Person::Anonymised(a) => a.clone(),
        }
    }
    pub fn archive(&mut self) {
        match self {
            Person::Detail(_) =>  {
                *self = Person::Anonymised(self.id())
            }
            _ => {}
        }
    }
}

#[derive(FromForm, Clone, Debug, Validate, Serialize, Deserialize, Encode, Default, Decode)]
pub struct BankDetails {
    #[validate(length(min = 10))]
    iban: String,
    bic: Option<String>,
    bank_name: Option<String>,
    account_name: Option<String>,
}

impl BankDetails {
    fn set(&mut self, key: &str, value: String) -> Result<(), String> {
        Ok(match key {
            "iban" => { self.iban = value; },
            "bic" => { self.bic = Some(value); },
            "bank_name" => { self.bank_name = Some(value); },
            "account_name" => { self.account_name = Some(value); },
            _ => {
                return Err(format!("Unknown key {:} on bank info", key))
            }
        })
    }
}

#[derive(FromForm, Clone, Debug, Validate, Serialize, Deserialize, Encode, Default, Decode)]
pub struct GrantInfo { 
    /// grant info
    #[validate(range(min=1, max = 200))]
    amount: u32,
    #[validate(length(min = 5))]
    cost_breakdown: String,
}

impl GrantInfo {
    fn set(&mut self, key: &str, value: String) -> Result<(), String> {
        Ok(match key {
            "amount" => {
                self.amount = value
                    .parse()
                    .map_err(|e| format!("parsing amount failed: {:}", e))?;
            },
            "cost_breakdown" => {
                self.cost_breakdown = value;
            }
            _ => {
                return Err(format!("Unknown key {:} on grant info", key));
            }
        })
    }
}

#[derive(FromForm, Clone, Debug, Validate, Serialize, Deserialize, Encode, Default, Decode)]
pub struct ExtraInfo {
    comment: Option<String>,
    #[validate(required)]
    accepted_privacy: Option<bool>,
    #[validate(required)]
    accepted_coc: Option<bool>,
    newsletter_monthly: bool,
    newsletter_fund: bool
}

impl ExtraInfo {
    fn set(&mut self, key: &str, value: String) -> Result<(), String> {
        Ok(match key {
            "comment" => {
                self.comment = Some(value)
            },
            "accepted_privacy" => {
                self.accepted_privacy = Some(true);
            },
            "accepted_coc" => {
                self.accepted_coc = Some(true);
            },
            "newsletter_monthly" => {
                self.newsletter_monthly = true;
            },
            "newsletter_fund" => {
                self.newsletter_fund = true;
            }
            _ => {
                return Err(format!("Unknown key {:} on extra info", key));
            }
        })
    }
}


#[derive(FromForm, Clone, Debug, Validate, Serialize, Deserialize, Encode, Decode, Default)]
pub struct EventInfo {
    #[validate(length(min = 1))]
    name: String,
    #[validate(length(min = 20))]
    description: String,
    #[validate(length(min = 2))]
    organiser: String,
    url: Option<String>,
    #[validate(length(min = 5))]
    why: String,
}

impl EventInfo {
    fn set(&mut self, key: &str, value: String) -> Result<(), String> {
        Ok(match key {
            "name" => { self.name = value; },
            "description" => { self.description = value; },
            "organiser" => { self.organiser = value; },
            "url" => { self.url = Some(value); },
            "why" => { self.why = value; },
            _ => {
                return Err(format!("Unknown key {:} on event info", key));
            }
        })
    }
}

#[derive(Debug, Clone, Validate, Serialize, Deserialize, Default)]
pub struct AktivistiGrantForm {
    #[validate]
    grant_info: GrantInfo,
    identities: Vec<Identity>,
    #[validate]
    person: PersonalDetails,
    #[validate]
    bank: BankDetails,
    #[validate]
    extra: ExtraInfo,
}

impl<'f> FromForm<'f> for AktivistiGrantForm {
    // In practice, we'd use a more descriptive error type.
    type Error = String;

    fn from_form(items: &mut FormItems<'f>, strict: bool) -> Result<AktivistiGrantForm, Self::Error> {

        let mut s: Self = Default::default();

        for item in items {
            let (key, value) = item.key_value_decoded();
            if key.starts_with("grant_") {
                s.grant_info.set(&key[6..], value)?;
            } else if key.starts_with("id_") {
                s.identities.push(Identity::from(&key[2..])?);
            } else if key.starts_with("person_") {
                s.person.set(&key[7..], value)?;
            } else if key.starts_with("bank_") {
                s.bank.set(&key[5..], value)?;
            } else if key.starts_with("extra_") {
                s.extra.set(&key[6..], value)?;
            } else if strict {
                return Err(format!("Unknown key {:}", key));
            }
        }

        Ok(s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub enum Bank {
    Anonymised(String), // storing the hash of the E-Mail
    Detail(BankDetails), // FullInfo
}

impl Bank { 
    pub fn id(&self) -> String {
        match self {
            Bank::Detail(d) => {
                format!("{:x}", Blake2b::new()
                    .chain(d.iban.replace(" ", "").to_lowercase().as_bytes())
                    .finalize()
                )
            }
            Bank::Anonymised(a) => a.clone(),
        }
    }
    pub fn archive(&mut self) {
        match self {
            Bank::Detail(_) =>  {
                *self = Bank::Anonymised(self.id())
            }
            _ => {}
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct AktivistiGrantDetails {
    grant_info: GrantInfo,
    person: Person,
    identities: Vec<Identity>,
    bank: Bank,
    extra: ExtraInfo,
}

impl Archivable for AktivistiGrantDetails {
    fn archive(&mut self) {
        self.person.archive();
        self.bank.archive();
    }
}

impl From<AktivistiGrantForm>  for  AktivistiGrantDetails {
    fn from(a: AktivistiGrantForm) -> AktivistiGrantDetails {
        AktivistiGrantDetails {
            grant_info: a.grant_info,
            person: Person::Detail(a.person),
            identities: a.identities,
            bank: Bank::Detail(a.bank),
            extra: a.extra,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, Validate)]
pub struct EventGrantForm {
    #[validate]
    grant_info: GrantInfo,
    identities: Vec<Identity>,
    #[validate]
    event_info: EventInfo,
    #[validate]
    person: PersonalDetails,
    #[validate]
    bank: BankDetails,
    #[validate]
    extra: ExtraInfo,
}


impl<'f> FromForm<'f> for EventGrantForm {
    // In practice, we'd use a more descriptive error type.
    type Error = String;

    fn from_form(items: &mut FormItems<'f>, strict: bool) -> Result<EventGrantForm, Self::Error> {

        let mut s: Self = Default::default();

        for item in items {
            let (key, value) = item.key_value_decoded();
            if key.starts_with("grant_") {
                s.grant_info.set(&key[6..], value)?;
            } else if key.starts_with("id_") {
                s.identities.push(Identity::from(&key[3..])?);
            } else if key.starts_with("event_") {
                s.event_info.set(&key[6..], value)?;
            } else if key.starts_with("person_") {
                s.person.set(&key[7..], value)?;
            } else if key.starts_with("bank_") {
                s.bank.set(&key[5..], value)?;
            } else if key.starts_with("extra_") {
                s.extra.set(&key[6..], value)?;
            } else if strict {
                return Err(format!("Unknown key {:}", key));
            }
        }

        Ok(s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct EventGrantDetails {
    grant_info: GrantInfo,
    event_info: EventInfo,
    person: Person,
    identities: Vec<Identity>,
    bank: Bank,
    extra: ExtraInfo,
}

impl Archivable for EventGrantDetails {
    fn archive(&mut self) {
        self.person.archive();
        self.bank.archive();
    }
}

impl From<EventGrantForm> for EventGrantDetails {
    fn from(a: EventGrantForm) -> EventGrantDetails {
        EventGrantDetails {
            grant_info: a.grant_info,
            event_info: a.event_info,
            person: Person::Detail(a.person),
            identities: a.identities,
            bank: Bank::Detail(a.bank),
            extra: a.extra
        }
    }
}

/// The Usernames are numbered
pub type Username = String;
/// SINCE UNIX_EPOCH
pub type TimeStamp = u64;  

#[derive(Encode, Clone, Serialize, Deserialize, Decode, Debug)]
pub enum RejectionReason {
    /// We had no money to fund this anymore
    OutOfMoney,
    /// Ran against the Quota
    OutOfQuota,
    /// We had a formal reason, specified Option
	#[codec(index = "250")]
    Formal(String),
    /// We had anoter reason, specified Option
	#[codec(index = "251")]
    Other(String),
}

#[derive(Encode, Clone, Serialize, Deserialize, Decode, Debug)]
/// Archived States are anonymised
pub enum ArchivedState {
    /// This was accepted, amount X was paid – full Euros
    Funded(u32),
    /// This was retracted by the submitter
    Retracted,
    /// This was dismissed on formal grounds, information given
    Rejected(RejectionReason),
    /// This failed to be funded by the board
    Failed,
}

#[derive(Serialize, Clone, Deserialize, Encode, Decode, Debug)]
pub enum GrantState {
    /// Not shown until submitted
    Draft,
    /// This is in
    Incoming,
    Checking,
    Board,
    Accepted(u32),
    Paid(u32),
    Archived(ArchivedState)
}

impl GrantState {
    pub fn short_name(&self) -> &'static str {
        match self {
            GrantState::Draft => "draft",
            GrantState::Incoming => "incoming",
            GrantState::Checking => "checking",
            GrantState::Board => "board",
            GrantState::Accepted(_) => "accepted",
            GrantState::Paid(_) => "paid",
            GrantState::Archived(_) => "archived",
        }
    }
}

#[derive(Serialize, Clone, Deserialize, Encode, Decode, Debug)]
pub struct StateActivity {
    from: GrantState,
    to: GrantState,
    by: Username,
    when: TimeStamp,
    comment: Option<String>
}

pub trait Archivable {
    fn archive(&mut self);
}

#[derive(Serialize, Clone, Deserialize, Encode, Decode)]
pub struct GrantProcess<T: Encode + Decode + Archivable> {
    created: TimeStamp,
    last_updated: TimeStamp,
    title: String,
    state: GrantState,
    activities: Vec<StateActivity>,
    details: T
}

fn now() -> u64 {
    SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|s| s.as_secs())
            .expect("System time is always available. qed")
}

impl<T: Encode + Decode + Archivable> GrantProcess<T> {
    pub fn transition_to(&mut self, next_stage: &str, user: String, amount: u32, comment: Option<String>)
        -> Result<Option<(String, String)>, String>
    {
        let next = match (next_stage, &self.state) {
            ("draft", GrantState::Draft) |
            ("incoming", GrantState::Incoming) |
            ("checking", GrantState::Checking) |
            ("board", GrantState::Board) |
            ("accepted", GrantState::Accepted(_)) |
            ("paid", GrantState::Paid(_)) |
            ("archive", GrantState::Archived(_)) |
            ("rejected", GrantState::Archived(_)) |
            ("retracted", GrantState::Archived(_))|
            ("failed", GrantState::Archived(_)) => {
                // nothing to be done
                return Ok(None)
            },
            ("checking", _ ) => GrantState::Checking,
            ("board", _ ) => GrantState::Board,
            ("outofmoney", _ ) => GrantState::Archived(
                    ArchivedState::Rejected(
                        RejectionReason::OutOfMoney
                    )),
            ("outofquota", _ ) => GrantState::Archived(
                    ArchivedState::Rejected(
                        RejectionReason::OutOfQuota
                    )),
            ("reject", _ ) => GrantState::Archived(
                    ArchivedState::Rejected(
                        RejectionReason::Other(comment.clone().unwrap_or_default())
                    )),
            ("formal_reject", _ ) => GrantState::Archived(
                    ArchivedState::Rejected(
                        RejectionReason::Formal(comment.clone().unwrap_or_default())
                    )),
            ("failed", _ ) => GrantState::Archived(ArchivedState::Failed),
            ("retracted", _ ) => GrantState::Archived(ArchivedState::Retracted),
            ("accepted", _) => GrantState::Accepted(amount),
            ("paid", GrantState::Accepted(accepted)) => {
                GrantState::Paid(accepted.clone())
            },
            ("funded", GrantState::Accepted(stored)) |
            ("arcbive", GrantState::Paid(stored) ) => {
                GrantState::Archived(ArchivedState::Funded(stored.clone()))
            },
            ("funded", _ ) => {
                GrantState::Archived(ArchivedState::Funded(amount))
            },
            (key, cur) => {
                return Err(format!("Unsupported transition {:?} => {:}", cur, key))
            }
        };

        let res = match next {
            GrantState::Paid(_) => Some((
                "Dein Radikal*Fund Antrag wurde bewilligt, das Geld ist unterwegs".to_string(),
                "grants/emails/ausgezahlt".to_string()
            )),
            GrantState::Archived(ArchivedState::Rejected(_)) => Some((
                "Dein Radikal*Fund Antrag wurde abgelehnt".to_string(),
                "grants/emails/abgelehnt".to_string()
            )),
            GrantState::Archived(ArchivedState::Failed) => Some((
                "Dein Radikal*Fund Antrag wurde nicht gefundet".to_string(),
                "grants/emails/not_funded".to_string()
            )),
            _ => None
        };

        match next {
            GrantState::Archived(_) => {
                self.details.archive();
            },
            _ => {}
        };

        let when = now();
        self.last_updated = when.clone();
        let from = self.state.clone();
        self.state = next.clone();
        let act = StateActivity {
            from,
            to: next,
            by: user,
            when,
            comment
        };

        self.activities.insert(0, act);
        Ok(res)
    } 
}

impl<T> From<T> for GrantProcess<T>
    where T: Encode + Decode + Archivable
{
    fn from(t: T) -> Self {
        let now = now();

        Self {
            created: now.clone(),
            last_updated: now,
            title: make_random_title(5),
            state: GrantState::Incoming,
            activities: vec![],
            details: t,
        }
    }
}

#[derive(FromForm, Debug)]
pub struct NextStageForm {
    next: String,
    comment: Option<String>,
    send_mail: bool,
}

#[derive(Serialize, Deserialize, Encode, Decode)]
pub enum Model {
    AktivistiGrant(GrantProcess<AktivistiGrantDetails>),
    EventGrant(GrantProcess<EventGrantDetails>),
}

impl Model {
    pub fn state_name(&self) -> Option<&'static str> {
        match self {
            Model::EventGrant(g) => Some(g.state.short_name()),
            Model::AktivistiGrant(g) => Some(g.state.short_name()),
            _ => None
        }
    }

    pub fn next_stage(&mut self, who: String, next: NextStageForm)
        -> Result<Option<(String, String)>, String>
    {
        match self {
            Model::EventGrant(process) =>
                process.transition_to(
                    &next.next,
                    who,
                    process.details.grant_info.amount,
                    next.comment),
            Model::AktivistiGrant(process) => 
                process.transition_to(
                    &next.next,
                    who,
                    process.details.grant_info.amount,
                    next.comment),
            _ => Err("Type unsupported for this activity".into())
        }
    }

    pub fn title(&self) -> Option<&String> {
        match self {
            Model::EventGrant(g) => Some(&g.title),
            Model::AktivistiGrant(g) => Some(&g.title),
            _ => None
        }
    }

    pub fn get_addr_info(&self) -> Option<(String, String)> {
        match self {
            Model::EventGrant(g) => g.details.person.get_addr_info(),
            Model::AktivistiGrant(g) => g.details.person.get_addr_info(),
            _ => None
        }
    }

    pub fn get_rel_ids(&self) -> (String, String) {
        match self {
            Model::EventGrant(g) => { (g.details.person.id(), g.details.bank.id()) },
            Model::AktivistiGrant(g) =>  { (g.details.person.id(), g.details.bank.id()) },
        }
    }
}

impl From<AktivistiGrantForm> for Model {
    fn from(a: AktivistiGrantForm) -> Model {
        Self::AktivistiGrant(GrantProcess::from(AktivistiGrantDetails::from(a)))
    }
}

impl From<EventGrantForm> for Model {
    fn from(a: EventGrantForm) -> Model {
        Self::EventGrant(GrantProcess::from(EventGrantDetails::from(a)))
    }
}