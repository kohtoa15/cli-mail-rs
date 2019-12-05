extern crate openssl;
extern crate pop3;

use std::{
    net::TcpStream,
    collections::HashMap,
    cmp::{
        PartialEq,
        PartialOrd,
        Ordering,
    },
};

use openssl::{
    ssl::{SslConnectorBuilder, SslMethod},
};
use pop3::{
    POP3Stream,
    POP3Result,
};
use imap::{
    Client as ImapClient,
    Session as ImapSession,
    types::{
        Fetch,
        ZeroCopy,
    },
};
use native_tls::{
    TlsConnector,
    TlsStream,
};
use datetime::{
    OffsetDateTime,
};

use super::inbox::{
    Mail,
    MailBuilder,
};
use super::account::{
    InboxConfig,
};
use super::util;
use super::decoder;


pub struct MailProxy {
    header: MailHeader,
    mail: Option<Mail>,
}

impl MailProxy {
    pub fn from_header(header: MailHeader) -> MailProxy {
        MailProxy {
            header,
            mail: None,
        }
    }

    pub fn get_info(&self) -> String {
        return if let Some(mail) = &self.mail {
            mail.get_info()
        } else {
            self.header.get_info()
        }
    }

    pub fn get_mail(&mut self, adapter: &mut InboxAdapter) -> Option<&Mail> {
        // Check if Mail has already been loaded
        if let None = &self.mail {
            // Load Mail
            println!("Mail must be loaded!");
            self.mail = adapter.get_mail(&self.header);
        }
        // If loading was successful, return mail
        return if let Some(mail) = &self.mail {
            println!("Returning mail ...");
            Some(mail)
        } else {
            None
        }
    }
}

impl Eq for MailProxy {}

impl PartialEq for MailProxy {
    fn eq(&self, other: &MailProxy) -> bool {
        self.header == other.header
    }
}

impl PartialOrd for MailProxy {
    fn partial_cmp(&self, other: &MailProxy) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MailProxy {
    fn cmp(&self, other: &MailProxy) -> Ordering {
        self.header.cmp(&other.header)
    }
}

pub struct MailHeader {
    id: u32,
    to: String,
    from: String,
    date: Option<OffsetDateTime>,
    subject: String,
}

impl Eq for MailHeader {}

impl PartialEq for MailHeader {
    fn eq(&self, other: &MailHeader) -> bool {
        self.id == other.id
    }
}

impl PartialOrd for MailHeader {
    fn partial_cmp(&self, other: &MailHeader) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MailHeader {
    fn cmp(&self, other: &MailHeader) -> Ordering {
        match (self.date, other.date) {
            (Some(own_date), Some(other_date)) => util::compare_date(&own_date, &other_date),
            (Some(date), _) => Ordering::Greater,
            (_, Some(date)) => Ordering::Less,
            (_, _) => Ordering::Equal,
        }
    }
}

impl MailHeader {
    pub fn new(id: u32, map: HashMap<String, String>) -> MailHeader {
        let to = map.get(&String::from("To")).map(|x| x.clone()).unwrap_or(String::from("<to>"));
        let from = map.get(&String::from("From")).map(|x| x.clone()).unwrap_or(String::from("<from>"));
        let date = match map.get(&String::from("Date")) {
            Some(date_str) => match decoder::decode_date(date_str) {
                Some(date) => Some(date),
                None => None,
            },
            None => None,
        };
        let raw = map.get(&String::from("Subject")).map(|x| x.clone().replace("\n", "").replace("\r", "")).unwrap_or(String::from("<subject>"));
        let subject = decoder::decode(raw);

        MailHeader {
            id, to, from, date, subject
        }
    }

    pub fn from_fetch(seq: u32, fetch: ZeroCopy<Vec<Fetch>>) -> MailHeader {
        let result = fetch.iter().next().unwrap();
        let content = result.header().map(|x| String::from_utf8(x.to_vec()).unwrap()).unwrap_or(String::new());
        let map = extract_mapping(content.clone());
        MailHeader::new(seq, map)
    }

    pub fn to_mail(&self) -> MailBuilder {
        let mut builder = MailBuilder::new();
        if let Some(date) = self.date {
            builder.date(date);
        }
        builder.from(self.from.clone()).subject(self.subject.clone()).to(vec![self.to.clone()]);
        return builder;
    }

    pub fn get_info(&self) -> String {
        format!("{} |  {} |  {}", util::fit_string_to_size(&self.date.map(|x| util::format_date(&x)).unwrap_or(String::from("<date>")), 20), util::fit_string_to_size(&self.from, 60), util::fit_string_to_size(&self.subject, 100))
    }
}

pub enum InboxAdapter {
    Pop3(Pop3Account),
    Imap(ImapAccount),
}

impl InboxAdapter {
    pub fn connect(config: &InboxConfig) -> std::io::Result<InboxAdapter> {
        match config {
            InboxConfig::Pop3(domain, port) => {
                let con = Pop3Account::connect(domain, *port)?;
                Ok(InboxAdapter::Pop3(con))
            },
            InboxConfig::Imap(domain, port) => {
                let con = ImapAccount::connect(domain, *port)?;
                Ok(InboxAdapter::Imap(con))
            }
        }
    }

    pub fn login(&mut self, username: &String, password: &String) -> bool {
        match self {
            InboxAdapter::Pop3(pop3) => pop3.login(username, password),
            InboxAdapter::Imap(imap) => imap.login(username, password),
        }
    }

    pub fn load_inbox(&mut self) -> Option<Vec<MailHeader>> {
        match self {
            InboxAdapter::Pop3(pop3) => pop3.load_inbox(),
            InboxAdapter::Imap(imap) => imap.load_inbox(),
        }
    }

    pub fn get_mail(&mut self, header: &MailHeader) -> Option<Mail> {
        match self {
            InboxAdapter::Pop3(pop3) => pop3.get_mail(header),
            InboxAdapter::Imap(imap) => imap.get_mail(header),
        }
    }
}

pub trait MailInbox {
    fn connect(domain: &String, port: u16) -> std::io::Result<Self> where Self: Sized;

    fn login(&mut self, username: &String, password: &String) -> bool;

    fn load_inbox(&mut self) -> Option<Vec<MailHeader>>;

    fn get_mail(&mut self, header: &MailHeader) -> Option<Mail>;
}

pub struct Pop3Account {
    stream: POP3Stream,
}

impl MailInbox for Pop3Account {
    fn connect(domain: &String, port: u16) -> std::io::Result<Pop3Account> {
        let connector = SslConnectorBuilder::new(SslMethod::tls()).unwrap().build();
        let stream = POP3Stream::connect((domain.as_str(), port), Some(connector), domain.as_str())?;
        Ok(Pop3Account {
            stream,
        })
    }

    fn login(&mut self, username: &String, password: &String) -> bool {
        let success = match self.stream.login(username.as_str(), password.as_str()) {
            POP3Result::POP3Ok => true,
            _ => false,
        };
        success
    }

    fn load_inbox(&mut self) -> Option<Vec<MailHeader>> {
        let mut ret = None;
        if self.stream.is_authenticated {
            ret = match self.stream.uidl(None) {
                POP3Result::POP3Uidl{ emails_metadata } => Some(emails_metadata.iter().map(|x| MailHeader::new(x.message_id as u32, HashMap::new())).collect()),
                _ => None,
            }
        }
        return ret;
    }

    fn get_mail(&mut self, header: &MailHeader) -> Option<Mail> {
        let mut ret = None;
        if self.stream.is_authenticated {
            match self.stream.retr(header.id as i32) {
                // ToDo: Convert raw msg to Mail ??
                POP3Result::POP3Message{ raw } => {},
                _ => {}
            };
        }
        return ret;
    }
}

enum ImapConnection {
    Client(ImapClient<TlsStream<TcpStream>>),
    Session(ImapSession<TlsStream<TcpStream>>),
    None,   // Only for Type Swapping
}

impl ImapConnection {
    pub fn get_session(self, username: &str, password: &str) -> ImapConnection {
        return match self {
            ImapConnection::Client(client) => {
                match client.login(username, password) {
                    Ok(session) => return ImapConnection::Session(session),
                    Err((e, client)) => {
                        println!("Could not log in on Imap Client: {}", e);
                        ImapConnection::Client(client)
                    }
                }
            },
            ImapConnection::Session(session) => ImapConnection::Session(session),
            ImapConnection::None => ImapConnection::None,
        }
    }

    pub fn is_session(&self) -> bool {
        return match self {
            ImapConnection::Session(_) => true,
            _ => false,
        };
    }
}

pub struct ImapAccount {
    imap: ImapConnection,
}

impl MailInbox for ImapAccount {
    fn connect(domain: &String, port: u16) -> std::io::Result<ImapAccount> {
        //Err(std::io::Error::from(std::io::ErrorKind::Other))
        let tls = TlsConnector::builder().build().unwrap();
        let client = imap::connect((domain.as_str(), port), domain, &tls).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        let imap = ImapAccount {
            imap: ImapConnection::Client(client),
        };
        Ok(imap)
    }

    fn login(&mut self, username: &String, password: &String) -> bool {
        let imap = std::mem::replace(&mut self.imap, ImapConnection::None);
        self.imap = imap.get_session(username.as_str(), password.as_str());
        self.imap.is_session()
    }

    fn load_inbox(&mut self) -> Option<Vec<MailHeader>> {
        if let ImapConnection::Session(session) = &mut self.imap {
            // Select Inbox
            return match session.select("INBOX") {
                Ok(_) => {
                    // Get unread mails
                    let unread = match session.search("UNSEEN SINCE 1-Dec-2019") {
                        Ok(val) => val.iter().map(|i| *i).collect::<Vec<u32>>(),
                        Err(e) => {
                            println!("Could not get unread mails: {}", e);
                            return None;
                        }
                    };
                    // Get other mails
                    let other = match session.search("SEEN SINCE 1-Dec-2019") {
                        Ok(val) => val.iter().map(|i| *i).collect::<Vec<u32>>(),
                        Err(e) => {
                            println!("Could not get other mails: {}", e);
                            return None;
                        }
                    };

                    // Combine to proto-mail-vec
                    let mut mails: Vec<(u32, bool)> = unread.into_iter().map(|x| (x, true)).collect();
                    mails.append(&mut other.into_iter().map(|x| (x, false)).collect());

                    // Get mail info for each identifier
                    let mut ret = Vec::new();
                    for (seq, _) in mails.into_iter() {
                        match session.fetch(format!("{}", seq).as_str(), "BODY.PEEK[HEADER]") {
                            Ok(res) => ret.push(MailHeader::from_fetch(seq, res)),
                            Err(e) => {
                                println!("Could not fetch mail: [{}]", e);
                                return None;
                            },
                        }
                    }
                    Some(ret)
                },
                Err(_) => None,
            }
        }
        None
    }

    fn get_mail(&mut self, header: &MailHeader) -> Option<Mail> {
        if let ImapConnection::Session(session) = &mut self.imap {
            // Select Inbox
            println!("Session open!");
            return match session.select("INBOX") {
                Ok(_) => {
                    // Fetch mail with specified identifier
                    println!("Inbox selected!");
                    match session.fetch(format!("{}", header.id).as_str(), "BODY[TEXT]") {
                        Ok(res) => {
                            println!("Fetched mail!");
                            let mut builder = header.to_mail();
                            // Append Text
                            if let Some(fetch) = res.get(0) {
                                println!("Got fetch!");
                                if let Some(bytes) = fetch.text() {
                                    println!("Got text!");
                                    if let Ok(text) = String::from_utf8(bytes.to_vec()) {
                                        println!("Parsed text!");
                                        builder.text(text);
                                    }
                                }
                            }
                            // Build mail
                            println!("Building mail ...");
                            match builder.build() {
                                Ok(mail) => Some(mail),
                                Err((_, field)) => {
                                    println!("Could not build mail: [missing field: {}]", field);
                                    None
                                }
                            }
                        },
                        Err(e) => {
                            println!("Could not fetch mail: [{}]", e);
                            None
                        },
                    }
                },
                Err(_) => {
                    println!("Couldn't select inbox!");
                    None
                },
            }
        }
        println!("No session established!");
        None
    }
}

fn extract_mapping(content: String) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut buf_key = String::new();
    let mut buf_val = String::new();

    let mut search_key = true;
    let mut prev = '0';
    for c in content.chars() {
        if search_key {
            if c == ':' {
                search_key = false;
            } else {
                buf_key.push(c);
            }
        } else {
            // If nextline without space after -> Next Key/Value
            if prev == '\n' && c != ' ' {
                // Insert K/V
                map.insert(buf_key.clone(), buf_val.trim_end().to_string());
                buf_key.clear();
                buf_val.clear();
                // Switch mode
                search_key = true;
                buf_key.push(c);
            } else if prev != ':' {
                buf_val.push(c);
            }
        }
        prev = c;
    }
    map.insert(buf_key, buf_val.trim_end().to_string());
    return map;
}
