
use lettre::stub::StubTransport;
use lettre::file::FileTransport;
use lettre::smtp::{SmtpTransport, ConnectionReuseParameters};
use lettre::sendmail::SendmailTransport;
use lettre::smtp::authentication::{Credentials, Mechanism};
use lettre::SmtpClient;
pub use lettre::{Transport, SendableEmail};
pub use lettre_email::{Mailbox, Email, EmailBuilder};
use rocket::config::Table;
use std::str::FromStr;
use parking_lot::Mutex;

use rocket_contrib::templates::Template;

/// Wrapper around various lettre::Transport implementations
pub struct EmailSender(Mutex<SenderInner>);
enum SenderInner{
    Stub(StubTransport, String),
    Sendmail(SendmailTransport, String),
    File(FileTransport, String),
    SMTP(SmtpTransport, String),
}

impl SenderInner {

    /// Finish the email (set `from`) and send it with the configured transport
    fn send(&mut self, builder: EmailBuilder) -> Result<(), String> {
        let build = |addr: &str| ->  Result<SendableEmail, String> { builder
            .from(addr)
            .build()
            .map(|e| e.into())
            .map_err(|e| format!("Building Email failed: {}", e))
        };

        match self {
            SenderInner::Stub(t, addr) => t.send(build(&addr)?).map_err(|_| unreachable!("never fails")),
            SenderInner::Sendmail(t, addr) => t.send(build(&addr)?).map_err(|e| format!("Sending mail failed: {}", e)),
            SenderInner::File(t, addr)  => t.send(build(&addr)?).map_err(|e| format!("Sending mail failed: {}", e)),
            SenderInner::SMTP(t, addr) => t.send(build(&addr)?).map(|_| ()).map_err(|e| format!("Sending mail failed: {}", e)),
        }
    }

}

impl Default for EmailSender { 
    fn default() -> Self {
        Self::default_with_from("grantee@example.org".into())
    }
 }

impl EmailSender {

    fn default_with_from(from: String) ->Self {
        Self(Mutex::new(SenderInner::Stub(StubTransport::new_positive(), from)))
    }

    pub fn send(&self, builder: EmailBuilder) -> Result<(), String> {
        self.0.lock().send(builder)
    }

}


pub fn make_lettre_transport(table: &Table) -> Result<EmailSender, String>
{
    let transport = table.get("mail").and_then(|s| s.as_str()).unwrap_or("stub");
    let from = table.get("from")
        .and_then(|s| s.as_str())
        .unwrap_or("ben@example.org")
        .to_string();
    Ok(match transport {
        "file" => {
            let file = table.get("file").and_then(|s| s.as_str()).unwrap_or("emails.log");
            EmailSender(Mutex::new(SenderInner::File(FileTransport::new(file), from)))
        },
        "sendmail" => EmailSender(Mutex::new(SenderInner::Sendmail(SendmailTransport::new(), from))),
        "smtp" => {
            let mut client = if let Some(hostname) = table.get("host").and_then(|s| s.as_str()) {
                SmtpClient::new_simple(&hostname)
                    .map_err(|e| format!("Could not build SMTP client: {}", e))?
            } else {
                SmtpClient::new_unencrypted_localhost()
                    .map_err(|e| format!("Could not build SMTP client: {}", e))?
            }.smtp_utf8(table.get("utf8").and_then(|s| s.as_bool()).unwrap_or(true));

            let username = table.get("username").and_then(|s| s.as_str());
            let password = table.get("password").and_then(|s| s.as_str());

            if username.is_some() || password.is_some() {
                client = client.credentials(Credentials::new(
                    username.unwrap_or_default().to_string(),
                    password.unwrap_or_default().to_string()
                ));
            }

            if let Some(auth) = table.get("auth").and_then(|s| s.as_str()) {
                client = client.authentication_mechanism(match auth {
                    "plain" => Mechanism::Plain, 
                    "login" => Mechanism::Login,
                    "xoauth2" => Mechanism::Xoauth2,
                    e => return Err(format!("Unknown auth mechanism: {:}", e))
                });
            }

            if let Some(connection_reuse) = table.get("connection_reuse").and_then(|s| s.as_str()) {
                let reuse = if let Ok(v) = u16::from_str(connection_reuse) {
                    ConnectionReuseParameters::ReuseLimited(v)
                } else {
                    match connection_reuse {
                        "true" => ConnectionReuseParameters::ReuseUnlimited,
                        "false" | "no" => ConnectionReuseParameters::NoReuse,
                        e =>  return Err(format!("Unknown reuse param: {:}", e))
                    }
                };
                client = client.connection_reuse(reuse);
            }

            EmailSender(Mutex::new(SenderInner::SMTP(client.transport(), from)))

        }
        _ => EmailSender::default_with_from(from)
    })
}

pub fn send_email<I: Into<Mailbox>>(sender: &EmailSender, to: I, subject: String, html: String)
    -> Result<(), String>
{
    let mail = Email::builder()
        .to(to.into())
        .subject(subject)
        .html(html);
    
    sender.send(mail)
}