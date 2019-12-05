extern crate serde_yaml;
extern crate serde;

use std::{
    collections::HashMap,
    fs::File,
    error::Error,
};
use super::account::{
    Account,
};
use super::mail::{
    InboxAdapter,
    MailProxy,
    MailHeader,
};

use datetime::{
    OffsetDateTime,
    Offset,
    LocalDateTime,
};

#[derive(Clone)]
pub struct MailBuilder {
    date: Option<OffsetDateTime>,
    from: Option<String>,
    to: Option<Vec<String>>,
    cc: Option<Vec<String>>,
    bcc: Option<Vec<String>>,
    subject: Option<String>,
    text: Option<String>,
}

impl MailBuilder {
    pub fn new() -> MailBuilder {
        MailBuilder {
            date: None,
            from: None,
            to: None,
            cc: None,
            bcc: None,
            subject: None,
            text: None,
        }
    }

    pub fn date(&mut self, val: OffsetDateTime) -> &mut MailBuilder {
        self.date = Some(val);
        self
    }

    pub fn from(&mut self, val: String) -> &mut MailBuilder {
        self.from = Some(val);
        self
    }

    pub fn to(&mut self, val: Vec<String>) -> &mut MailBuilder {
        self.to = Some(val);
        self
    }

    pub fn cc(&mut self, val: Vec<String>) -> &mut MailBuilder {
        self.cc = Some(val);
        self
    }

    pub fn bcc(&mut self, val: Vec<String>) -> &mut MailBuilder {
        self.bcc = Some(val);
        self
    }

    pub fn subject(&mut self, val: String) -> &mut MailBuilder {
        self.subject = Some(val);
        self
    }

    pub fn text(&mut self, val: String) -> &mut MailBuilder {
        self.text = Some(val);
        self
    }

    pub fn build(self) -> Result<Mail, (MailBuilder, String)> {
        let cloned = self.clone();
        let mail = Mail {
            date: self.date.unwrap_or(Offset::of_hours_and_minutes(1, 0).unwrap().transform_date(LocalDateTime::now())),
            from: self.from.ok_or((cloned.clone(), String::from("from")))?,
            to: self.to.ok_or((cloned.clone(), String::from("to")))?,
            cc: self.cc.unwrap_or(Vec::new()),
            bcc: self.bcc.unwrap_or(Vec::new()),
            subject: self.subject.ok_or((cloned.clone(), String::from("about")))?,
            text: self.text.ok_or((cloned.clone(), String::from("text")))?,
        };
        Ok(mail)
    }

    pub fn show_preview(&self) {
        let null_str = String::from("<null>");
        println!("From:\t{}", self.from.clone().unwrap_or(null_str.clone()));
        println!("To:\t{}", self.to.clone().map(|x| x.join(", ")).unwrap_or(null_str.clone()));
        println!("Cc:\t{}", self.cc.clone().map(|x| x.join(", ")).unwrap_or(null_str.clone()));
        println!("Bcc:\t{}", self.bcc.clone().map(|x| x.join(", ")).unwrap_or(null_str.clone()));
        println!("About:\t{}", self.subject.clone().unwrap_or(null_str.clone()));
        println!("Text:\n{}", self.text.clone().unwrap_or(null_str.clone()));
    }
}

pub struct Mail {
    date: OffsetDateTime,
    pub from: String,
    to: Vec<String>,
    cc: Vec<String>,
    bcc: Vec<String>,
    pub subject: String,
    text: String,
}

impl Mail {
    pub fn get_info(&self) -> String {
        let mut ret = String::new();
        ret.push_str(self.from.as_str());
        ret.push_str(" | ");
        ret.push_str(self.subject.as_str());
        return ret;
    }

    pub fn print_all(&self) {
        println!("From:\t{}", self.from);
        println!("To:\t{}", self.to.join(", "));
        println!("Cc:\t{}", self.cc.join(", "));
        println!("Bcc:\t{}", self.bcc.join(", "));
        println!("Subject:\t{}", self.subject);
        println!("Text:\n{}", self.text);
    }
}

pub struct Inbox {
    mails: Vec<(MailProxy, bool)>,
    account: Account,
    opened_mail: Option<usize>,
    input: Option<InboxAdapter>,
}

impl Inbox {
    pub fn new(account: Account) -> Inbox {
        Inbox {
            mails: Vec::new(),
            account,
            opened_mail: None,
            input: None,
        }
    }

    pub fn get_account_name(&self) -> String {
        self.account.name.clone()
    }

    // Returns number of new mails
    pub fn refresh(&mut self) -> usize {
        let mut num: usize = 0;
        // Init InboxAdapter, if not yet initiated
        if self.input.is_none() {
            println!("Initiating InboxAdapter ...");
            let adapter = self.account.get_inbox_adapter();
            if let Err(e) = &adapter {
                println!("Could not refresh inbox for \"{}\" [{}]", self.account.name, e);
            }
            self.input = adapter.ok();
        }
        // Load Inbox if Adapter is valid
        if let Some(adapter) = &mut self.input {
            println!("Loading with Adapter ...");
            if let Some(vec) = adapter.load_inbox() {
                println!("Load inbox successful ...");
                let mut loaded: Vec<(MailProxy, bool)> = vec.into_iter().map(|x| (MailProxy::from_header(x), true)).collect();
                num += loaded.len();
                self.mails.append(&mut loaded);
            }
        }
        self.mails.sort_by(|(a, _), (b, _)| a.cmp(b));

        return num;
    }

    pub fn print_account(&self) {
        self.account.print();
    }

    pub fn show_mails(&self, named: bool) {
        if self.mails.is_empty() {
            println!("No mails in inbox of \"{}\"", self.get_account_name());
        } else {
            if named {
                println!("\"{}\"", self.get_account_name());
            }
            self.mails.iter().for_each(|(m, _)| println!("\t{}", m.get_info()));
        }
    }

    pub fn show_unread(&self, named: bool) {
        let unread: Vec<&MailProxy> = self.mails.iter().filter(|(_, unread)| *unread).map(|(m, _)| m).collect();
        if unread.is_empty() {
            println!("No unread mails in inbox!");
        } else {
            if named {
                println!("\"{}\"", self.get_account_name());
            }
            unread.iter().for_each(|m| println!("\t{}", m.get_info()));
        }
    }

    pub fn open_mail(&mut self, ident: String) {
        // Check if ident is int
        let index;
        if let Ok(id) = ident.parse::<usize>() {
            if id < self.mails.len() {
                index = Some(id);
            } else {
                index = None;
            }
        } else {
            // Check how many chars match with MailHead
            let id = self.mails.iter().map(|(m, _)| {
                m.get_info().chars().zip(ident.chars()).enumerate().find(|(_, (m, o))| m != o).map_or(0, |(i, _)| i);
            }).enumerate().max_by(|(_, a), (_, b)| a.cmp(b)).map(|(i, _)| i);
            index = id;
        }
        self.opened_mail = index;

        // Set mail unread false
        if let Some(id) = self.opened_mail {
            self.mails.get_mut(id).unwrap().1 = false;
        }
    }

    pub fn get_opened_mail(&mut self) -> Option<&Mail> {
        let opened_mail = self.opened_mail.clone();
        return if let Some(ident) = opened_mail {
            match &mut self.input {
                Some(adapter) => self.mails.get_mut(ident).unwrap().0.get_mail(adapter),
                None => None,
            }
        } else {
            None
        }
    }
}

pub struct InboxManager {
    account_file: String,
    accounts: HashMap<String, Inbox>,
    drafts_folder: String,
    pub opened_inbox: Option<String>,
    pub current_mail_writing: Option<MailBuilder>,
}

impl InboxManager {
    pub fn new(account_file: String) -> InboxManager {
        InboxManager {
            account_file,
            accounts: HashMap::new(),
            drafts_folder: String::new(),
            opened_inbox: None,
            current_mail_writing: None,
        }
    }

    pub fn load_file(&mut self) -> Result<(), Box<dyn Error>>  {
        let file = File::open(self.account_file.clone())?;
        let accounts: Vec<Account> = serde_yaml::from_reader(file)?;
        self.accounts = HashMap::with_capacity(accounts.len());
        for account in accounts.clone().into_iter() {
            let ident = match account.shortcut.clone() {
                Some(s) => s,
                None => account.name.clone(),
            };
            self.accounts.insert(ident, Inbox::new(account));
        }
        return Ok(());
    }

    pub fn refresh(&mut self) {
        println!("Refreshing inboxes ...");
        // Refresh available account inboxes
        let mut total_count: usize = 0;
        for (key, acc) in self.accounts.iter_mut() {
            println!("Refresh account \"{}\"", key);
            let count = acc.refresh();
            total_count += count;
        }
        println!("{} new mails loaded!", total_count);
    }

    pub fn show_inbox(&self, ident: Option<String>) {
        if let Some(key) = ident {
            let account = self.accounts.get(&key);
            if let Some(account) = account {
                account.show_mails(true);
            } else {
                println!("no account named \"{}\" available!", key);
            }
        } else {
            // Show all inboxes
            self.accounts.iter().for_each(|(_, a)| a.show_mails(true));
        }
    }

    pub fn show_servers(&self) {
        println!("Displaying info for {} server{} ...", self.accounts.len(), match self.accounts.len() != 1 {
            true => "s",
            false => "",
        });
        self.accounts.iter().for_each(|(_, a)| a.print_account());
    }

    pub fn show_drafts(&self) {

    }

    pub fn open_inbox(&mut self, ident: String) -> bool {
        let valid = self.accounts.contains_key(&ident);
        if valid {
            self.opened_inbox = Some(ident);
        }
        return valid;
    }

    pub fn get_opened_inbox(&mut self) -> Option<&mut Inbox> {
        if let Some(opened) = &self.opened_inbox {
            if let Some(inbox) = self.accounts.get_mut(opened) {
                return Some(inbox);
            }
        }
        None
    }
}
