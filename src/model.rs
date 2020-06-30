
use parity_scale_codec::{Encode, Decode};
use rocket::request::{Form, FromForm, FormItems};
use serde::{Serialize, Deserialize};
use std::time::SystemTime;

// A trait that the Validate derive will impl
use validator::{Validate, ValidationError};

#[derive(Debug, Encode, Serialize, Deserialize, Decode)]
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
            "non_man" => Self::NonWhite,
            "with_disability" => Self::WithDisability,
            "inter" => Self::Inter,
            "agender" => Self::Agender,
            i => return Err(format!("Unknown key {:}", i)),
        })
    }
}


#[derive(FromForm, Debug, Validate, Serialize, Deserialize, Encode, Default, Decode)]
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

#[derive(Debug, Encode, Decode, Serialize, Deserialize)]
enum Person {
    Anonymised(String), // storing the hash of the E-Mail
    Detail(PersonalDetails), // FullInfo
}

#[derive(FromForm, Debug, Validate, Serialize, Deserialize, Encode, Default, Decode)]
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

#[derive(FromForm, Debug, Validate, Serialize, Deserialize, Encode, Default, Decode)]
pub struct GrantInfo { 
    /// grant info
    #[validate(range(max = 200))]
    amount: u8,
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

#[derive(FromForm, Debug, Validate, Serialize, Deserialize, Encode, Default, Decode)]
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


#[derive(FromForm, Debug, Validate, Serialize, Deserialize, Encode, Decode, Default)]
pub struct EventInfo {
    #[validate(length(min = 5))]
    name: String,
    #[validate(length(min = 20))]
    description: String,
    #[validate(length(min = 5))]
    organiser: String,
    #[validate(length(min = 5))]
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

#[derive(Debug, Validate, Serialize, Deserialize, Default)]
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
            let key = item.key.as_str();
            println!("{:} : {:}", key, item.value);
            if key.starts_with("grant_") {
                s.grant_info.set(&item.key[6..], item.value.to_string())?;
            } else if key.starts_with("id_") {
                s.identities.push(Identity::from(&item.key[2..])?);
            } else if key.starts_with("person_") {
                s.person.set(&item.key[7..], item.value.to_string())?;
            } else if key.starts_with("bank_") {
                s.bank.set(&item.key[5..], item.value.to_string())?;
            } else if key.starts_with("extra_") {
                s.extra.set(&item.key[6..], item.value.to_string())?;
            } else if strict {
                return Err(format!("Unknown key {:}", key));
            }
        }

        Ok(s)
    }
}

#[derive(Debug, Serialize, Deserialize, Encode, Decode)]
pub struct AktivistiGrantDetails {
    grant_info: GrantInfo,
    person: Person,
    identities: Vec<Identity>,
    bank: Option<BankDetails>,
    extra: ExtraInfo,
}

impl From<AktivistiGrantForm>  for  AktivistiGrantDetails {
    fn from(a: AktivistiGrantForm) -> AktivistiGrantDetails {
        AktivistiGrantDetails {
            grant_info: a.grant_info,
            person: Person::Detail(a.person),
            identities: a.identities,
            bank: Some(a.bank),
            extra: a.extra,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default, Validate)]
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
            let key = item.key.as_str();
            println!("{:} : {:}", key, item.value);
            if key.starts_with("grant_") {
                s.grant_info.set(&item.key[6..], item.value.to_string())?;
            } else if key.starts_with("id_") {
                s.identities.push(Identity::from(&item.key[3..])?);
            } else if key.starts_with("event_") {
                s.event_info.set(&item.key[6..], item.value.to_string())?;
            } else if key.starts_with("person_") {
                s.person.set(&item.key[7..], item.value.to_string())?;
            } else if key.starts_with("bank_") {
                s.bank.set(&item.key[5..], item.value.to_string())?;
            } else if key.starts_with("extra_") {
                s.extra.set(&item.key[6..], item.value.to_string())?;
            } else if strict {
                return Err(format!("Unknown key {:}", key));
            }
        }

        Ok(s)
    }
}

#[derive(Debug, Serialize, Deserialize, Encode, Decode)]
pub struct EventGrantDetails {
    grant_info: GrantInfo,
    event_info: EventInfo,
    person: Person,
    identities: Vec<Identity>,
    bank: Option<BankDetails>,
    extra: ExtraInfo,
}

impl From<EventGrantForm> for EventGrantDetails {
    fn from(a: EventGrantForm) -> EventGrantDetails {
        EventGrantDetails {
            grant_info: a.grant_info,
            event_info: a.event_info,
            person: Person::Detail(a.person),
            identities: a.identities,
            bank: Some(a.bank),
            extra: a.extra
        }
    }
}

/// The UserIds are numbered
pub type UserId = u8;
/// SINCE UNIX_EPOCH
pub type TimeStamp = u64;  

#[derive(Encode, Serialize, Deserialize, Decode, Debug)]
/// Archived States are anonymised
pub enum ArchivedState {
    /// This was accepted, amount X was paid – full Euros
    Accepted(u32),
    /// This was retracted by the submitter
    Retracted,
    /// This was on formal grounds
    Rejected,
    /// This failed to be funded by the board
    Failed,
}

#[derive(Serialize, Deserialize, Encode, Decode, Debug)]
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

#[derive(Serialize, Deserialize, Encode, Decode, Debug)]
pub struct StateActivity {
    from: GrantState,
    to: GrantState,
    by: UserId,
    when: TimeStamp,
    comment: Option<String>
}

#[derive(Serialize, Deserialize, Encode, Decode)]
pub struct GrantProcess<T: Encode + Decode> {
    created: TimeStamp,
    last_updated: TimeStamp,
    title: String,
    state: GrantState,
    activities: Vec<StateActivity>,
    details: T
}

impl<T> From<T> for GrantProcess<T>
    where T: Encode + Decode
{
    fn from(t: T) -> Self {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|s| s.as_secs())
            .expect("System time is always available. qed");

        Self {
            created: now.clone(),
            last_updated: now,
            title: "test title".into(),
            state: GrantState::Incoming,
            activities: vec![],
            details: t,
        }
    }

}

#[derive(Serialize, Deserialize, Encode, Decode)]
pub enum Model {
    AktivistiGrant(GrantProcess<AktivistiGrantDetails>),
    EventGrant(GrantProcess<EventGrantDetails>),
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