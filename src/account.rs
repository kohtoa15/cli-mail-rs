extern crate serde;
extern crate pop3;
extern crate openssl;

use serde::{
    de::{
        self,
        Deserialize,
        Deserializer,
        Visitor,
        SeqAccess,
        MapAccess,
    },
    Serialize,
    Serializer,
};
use pop3::{
    POP3Stream,
};
use openssl::{
    ssl::{SslConnectorBuilder, SslMethod},
};

use super::{
    mail::{
        MailInbox,
        Pop3Account,
        ImapAccount,
        InboxAdapter,
    }
};


const POP3_PORT: u16 = 995;
const IMAP_PORT: u16 = 993;

#[derive(Clone)]
pub enum InboxConfig {
    Pop3(String, u16),
    Imap(String, u16),
}

impl InboxConfig {
    pub fn new_pop3(domain: String) -> InboxConfig {
        return InboxConfig::Pop3(domain, POP3_PORT);
    }

    pub fn new_imap(domain: String) -> InboxConfig {
        return InboxConfig::Imap(domain, IMAP_PORT);
    }
}

#[derive(Clone)]
pub struct Account {
    pub inbox_domain: InboxConfig,
    pub smtp_domain: String,
    pub name: String,
    pub password: String,
    pub shortcut: Option<String>,
}

impl Account {
    pub fn new(inbox_domain: InboxConfig, smtp_domain: String, name: String, password: String, shortcut: Option<String>) -> Account {
        Account {
            inbox_domain, smtp_domain, name, password, shortcut,
        }
    }

    pub fn print(&self) {
        let inbox_domain = match &self.inbox_domain {
            InboxConfig::Pop3(domain, _) => format!("POP3 Domain:\t{}", domain),
            InboxConfig::Imap(domain, _) => format!("IMAP Domain:\t{}", domain),
        };
        println!("Account \"{}\"\n\t{}\n\tSMTP Domain:\t{}\n\tPassword:\t{}\n\tShortcut:\t{}", self.name, inbox_domain, self.smtp_domain, vec!['*'; self.password.len()].into_iter().collect::<String>(), if let Some(sc) = &self.shortcut { sc.clone() } else { String::from("-") });
    }

    pub fn get_inbox_adapter(&self) -> std::io::Result<InboxAdapter> {
        let mut adapter = InboxAdapter::connect(&self.inbox_domain);
        if let Ok(adptr) = &mut adapter {
            adptr.login(&self.name, &self.password);
        }
        adapter
    }
}

impl Serialize for Account {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("Account", 5)?;
        match &self.inbox_domain {
            InboxConfig::Pop3(domain, _ ) => state.serialize_field("pop3_domain", domain)?,
            InboxConfig::Imap(domain, _ ) => state.serialize_field("imap_domain", domain)?,
        };
        state.serialize_field("smtp_domain", &self.smtp_domain)?;
        state.serialize_field("name", &self.name)?;
        state.serialize_field("password", &self.password)?;
        if let Some(sc) = &self.shortcut {
            state.serialize_field("shortcut", &sc)?;
        }
        state.end()
    }
}

impl<'a> Deserialize<'a> for Account {
    fn deserialize<D>(deserializer: D) -> Result<Account, D::Error>
        where D: Deserializer<'a>,
    {
        enum Field { Pop3Domain, ImapDomain, SmtpDomain, Name, Password, Shortcut };

        impl<'a> Deserialize<'a> for Field {
            fn deserialize<D>(deserializer: D) -> Result<Field, D::Error>
                where D: Deserializer<'a>
            {
                struct FieldVisitor;

                impl <'a> Visitor<'a> for FieldVisitor {
                    type Value = Field;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                        formatter.write_str("`pop3_domain` or `imap_domain` or `smtp_domain` or `name` or `password` or `shortcut`")
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Field, E>
                        where E: de::Error
                    {
                        match value {
                            "pop3_domain" => Ok(Field::Pop3Domain),
                            "imap_domain" => Ok(Field::ImapDomain),
                            "smtp_domain" => Ok(Field::SmtpDomain),
                            "name" => Ok(Field::Name),
                            "password" => Ok(Field::Password),
                            "shortcut" => Ok(Field::Shortcut),
                            _ => Err(de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }

                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct AccountVisitor;

        impl<'a> Visitor<'a> for AccountVisitor {
            type Value = Account;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct Account")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<Account, V::Error>
                where V: SeqAccess<'a>
            {
                let pop3_domain = seq.next_element()?;
                let imap_domain = seq.next_element()?;
                let smtp_domain = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(1, &self))?;
                let name = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(2, &self))?;
                let password = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(3, &self))?;
                let shortcut = seq.next_element()?;

                let inbox_config = match (pop3_domain, imap_domain) {
                    (Some(domain), None) => InboxConfig::new_pop3(domain),
                    (None, Some(domain)) => InboxConfig::new_imap(domain),
                    (_, _) => return Err(de::Error::invalid_length(0, &self)),
                };

                Ok(Account::new(inbox_config, smtp_domain, name, password, shortcut))
            }

            fn visit_map<V>(self, mut map: V) -> Result<Account, V::Error>
                where V: MapAccess<'a>
            {
                let mut pop3_domain = None;
                let mut imap_domain = None;
                let mut smtp_domain = None;
                let mut name = None;
                let mut password = None;
                let mut shortcut = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Pop3Domain => {
                            if pop3_domain.is_some() {
                                return Err(de::Error::duplicate_field("pop3_domain"));
                            }
                            pop3_domain = Some(map.next_value()?);
                        },
                        Field::ImapDomain => {
                            if imap_domain.is_some() {
                                return Err(de::Error::duplicate_field("imap_domain"));
                            }
                            imap_domain = Some(map.next_value()?);
                        },
                        Field::SmtpDomain => {
                            if smtp_domain.is_some() {
                                return Err(de::Error::duplicate_field("smtp_domain"));
                            }
                            smtp_domain = Some(map.next_value()?);
                        },
                        Field::Name => {
                            if name.is_some() {
                                return Err(de::Error::duplicate_field("name"));
                            }
                            name = Some(map.next_value()?);
                        },
                        Field::Password => {
                            if password.is_some() {
                                return Err(de::Error::duplicate_field("password"));
                            }
                            password = Some(map.next_value()?);
                        },
                        Field::Shortcut => {
                            if shortcut.is_some() {
                                return Err(de::Error::duplicate_field("shortcut"));
                            }
                            shortcut = Some(map.next_value()?);
                        },
                    }
                }
                let inbox_domain = match (pop3_domain, imap_domain) {
                    (Some(domain), None) => InboxConfig::new_pop3(domain),
                    (None, Some(domain)) => InboxConfig::new_imap(domain),
                    (_, _) => return Err(de::Error::missing_field("inbox_domain")),
                };
                let smtp_domain = smtp_domain.ok_or_else(|| de::Error::missing_field("smtp_domain"))?;
                let name = name.ok_or_else(|| de::Error::missing_field("name"))?;
                let password = password.ok_or_else(|| de::Error::missing_field("password"))?;

                Ok(Account::new(inbox_domain, smtp_domain, name, password, shortcut))
            }
        }

        const FIELDS: &'static [&'static str] = &["pop3_domain", "imap_domain", "smtp_domain", "name", "password", "shortcut"];
        deserializer.deserialize_struct("Account", FIELDS, AccountVisitor)
    }
}
