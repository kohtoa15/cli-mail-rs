extern crate pop3;
extern crate openssl;
extern crate clitc;
extern crate console;
extern crate mime;

mod inbox;
mod account;
mod receiving;
mod util;
mod decoder;

use console::{
    Style
};

use std::{
    collections::HashMap,
    rc::Rc,
    fs::File,
    sync::{Arc, Mutex},
    cmp::{Eq, PartialEq},
    hash::Hash,
};
use clitc::{
    events::{Event, EventHandler, WhitespaceSplitter},
    params::{CliParameters},
};
use inbox::{
    InboxManager,
    MailBuilder,
};

const GLOBAL_PROMPT: &str = "cli-mail-rs";

#[derive(Clone, Hash)]
enum Mode {
    Exit,
    Global,
    Inbox,
    Write,
    Read,
}

impl Mode {
    pub fn get_prompt(&self, path: Option<String>) -> (String, u8) {
        use Mode::*;
        let mut ret = String::new();
        if let Some(s) = path {
            ret.push('\"');
            ret.push_str(s.as_str());
            ret.push('\"');
            ret.push('~');
        }
        let (s, code) = match self {
            Exit => ("", 0),
            Global => (">", 1),
            Inbox => ("#", 2),
            Write => ("µ", 3),
            Read => ("λ", 4),
        };
        ret.push_str(s);
        return (ret, code);
    }
}

impl Eq for Mode {}

impl PartialEq for Mode {
    fn eq(&self, other: &Self) -> bool {
        use Mode::*;
        match (self, other) {
            (Exit, Exit) => true,
            (Global, Global) => true,
            (Inbox, Inbox) => true,
            (Write, Write) => true,
            (Read, Read) => true,
            (_, _) => false,
        }
    }
}

type ContextHandle = Arc<Mutex<InboxManager>>;
type Emitter = (Mode, Option<String>);

fn init_modes() -> (Arc<Mutex<Option<Emitter>>>, HashMap<Mode, HashMap<String, Event<ContextHandle, Emitter>>>) {
    let mut states: HashMap<Mode, HashMap<String, Event<ContextHandle, Emitter>>> = HashMap::new();
    let handle = Arc::new(Mutex::new(None));

    // Global Emitter
    {
        let mut global = HashMap::new();
        global.insert(String::from("refresh"), Event::<ContextHandle, Emitter>::Callback(Rc::new(|handle, _| {
            // Parse args
            let mut context = handle.lock().unwrap();
            context.refresh();
        })));

        global.insert(String::from("show-inbox"), Event::<ContextHandle, Emitter>::Callback(Rc::new(|handle, args| {
            // Parsing args for proper use
            let mut account = args.get(&String::from("account")).map(|x| x.to_string());
            if let Some(val) = account.clone() {
                if val == "all" {
                    account = None;
                }
            }
            //  call Display inboxes from context
            let context = handle.lock().unwrap();
            context.show_inbox(account);
        })));

        global.insert(String::from("inbox"), Event::<ContextHandle, Emitter>::Emit(Arc::clone(&handle), Rc::new(|ctx_handle, emit_handle, args| {
            // Handle param
            let account = args.get(&String::from("account")).map(|x| x.to_string());
            if let Some(val) = account.clone() {
                let mut context = ctx_handle.lock().unwrap();
                // Change mode if successful
                if context.open_inbox(val.clone()) {
                    let mut emitter = emit_handle.lock().unwrap();
                    *emitter = Some((Mode::Inbox, Some(val)));
                } else {
                    println!("no account named \"{}\" available!", val);
                }
            } else {
                println!("inbox command needs valid account as parameter!");
            }
        })));

        global.insert(String::from("show-servers"), Event::<ContextHandle, Emitter>::Callback(Rc::new(|handle, _| {
            let context = handle.lock().unwrap();
            context.show_servers();
        })));

        global.insert(String::from("show-drafts"), Event::<ContextHandle, Emitter>::Callback(Rc::new(|handle, args| {
            // ToDo: Show Drafts functionality
            println!("show-drafts not yet implemented!");
        })));

        global.insert(String::from("add-server"), Event::<ContextHandle, Emitter>::Callback(Rc::new(|handle, args| {
            // ToDo: Add Server functionality
            println!("add-server not yet implemented!");
        })));

        global.insert(String::from("write"), Event::<ContextHandle, Emitter>::Emit(Arc::clone(&handle), Rc::new(|handle, emit_handle, _| {
            // Emit Write Emitter switch
            let mut emitter = emit_handle.lock().unwrap();
            *emitter = Some((Mode::Write, None));
        })));

        global.insert(String::from("exit"), Event::<ContextHandle, Emitter>::Emit(Arc::clone(&handle), Rc::new(|handle, emit_handle, _| {
            // Emit Exit signal
            let mut emitter = emit_handle.lock().unwrap();
            *emitter = Some((Mode::Exit, None));
        })));
        states.insert(Mode::Global, global);
    }

    // Inbox Emitter
    {
        let mut inbox = HashMap::new();
        inbox.insert(String::from("show-unread"), Event::<ContextHandle, Emitter>::Callback(Rc::new(|handle, _| {
            let mut context = handle.lock().unwrap();
            if let Some(inbox) = context.get_opened_inbox() {
                inbox.show_unread(false);
            }
        })));
        inbox.insert(String::from("show-all"), Event::<ContextHandle, Emitter>::Callback(Rc::new(|handle, _| {
            let mut context = handle.lock().unwrap();
            if let Some(inbox) = context.get_opened_inbox() {
                inbox.show_mails(false);
            }
        })));
        inbox.insert(String::from("open"), Event::<ContextHandle, Emitter>::Emit(Arc::clone(&handle), Rc::new(|ctx_handle, emit_handle, args| {
            let param = args.get(&String::from("ident")).map(|x| x.to_string());
            if let Some(param) = param {
                let mut context = ctx_handle.lock().unwrap();
                if let Some(inbox) = context.get_opened_inbox() {
                    inbox.open_mail(param.clone());
                    if let Some(mail) = inbox.get_opened_mail() {
                        // change mode to read
                        let mut emitter = emit_handle.lock().unwrap();
                        *emitter = Some((Mode::Read, Some(mail.get_info())));
                    } else {
                        println!("Could not open mail!");
                    }
                }
            } else {
                println!("command open needs valid parameter!");
            }
        })));
        inbox.insert(String::from("exit"), Event::<ContextHandle, Emitter>::Emit(Arc::clone(&handle), Rc::new(|_, emit_handle, _| {
            // Emit mode change -> global signal
            let mut emitter = emit_handle.lock().unwrap();
            *emitter = Some((Mode::Global, Some(GLOBAL_PROMPT.to_string())));
        })));
        states.insert(Mode::Inbox, inbox);
    }

    // Read Emitter
    {
        let mut read = HashMap::new();
        read.insert(String::from("show-mail"), Event::<ContextHandle, Emitter>::Callback(Rc::new(|ctx_handle, args| {
            let mut context = ctx_handle.lock().unwrap();
            if let Some(inbox) = context.get_opened_inbox() {
                if let Some(mail) = inbox.get_opened_mail() {
                    mail.print_all();
                }
            }
        })));

        read.insert(String::from("reply"), Event::<ContextHandle, Emitter>::Emit(Arc::clone(&handle), Rc::new(|ctx_handle, emit_handle, args| {
            let mut prompt_path = None;
            {
                // set from, to and about on reply mail
                let mut context = ctx_handle.lock().unwrap();
                if let Some(inbox) = context.get_opened_inbox() {
                    let name = inbox.get_account_name();
                    if let Some(recv_mail) = inbox.get_opened_mail().clone() {
                        // Craft reply MailBuilder
                        let reply = recv_mail.create_reply();
                        context.current_mail_writing = Some(reply);
                        prompt_path = Some(name);
                    }
                }
            }
            // Change mode to write
            let mut emitter = emit_handle.lock().unwrap();
            *emitter = Some((Mode::Write, prompt_path));
        })));

        read.insert(String::from("close"), Event::<ContextHandle, Emitter>::Emit(Arc::clone(&handle), Rc::new(|ctx_handle, emit_handle, args| {
            // Change mode to global or inbox (if open)
            let emitted;
            {
                let mut context = ctx_handle.lock().unwrap();
                if let Some(inbox) = context.get_opened_inbox() {
                    // Change mode to inbox
                    emitted = (Mode::Inbox, Some(inbox.get_account_name()));
                } else {
                    // Change mode to global
                    emitted = (Mode::Global, Some(GLOBAL_PROMPT.to_string()));
                }
            }
            let mut emitter = emit_handle.lock().unwrap();
            *emitter = Some(emitted);
        })));
        states.insert(Mode::Read, read);
    }

    // Write Emitter
    {
        let mut write = HashMap::new();
        write.insert(String::from("from"), Event::<ContextHandle, Emitter>::Callback(Rc::new(|handle, args| {
            if let Some(sender) = args.get(&String::from("sender")) {
                let sender = match sender {
                    clitc::params::ParamValue::String(s) => s.clone(),
                    _ => String::new(),
                };

                let mut context = handle.lock().unwrap();
                if let Some(mail) = &mut context.current_mail_writing {
                    mail.from(sender);
                }
            }
        })));
        write.insert(String::from("to"), Event::<ContextHandle, Emitter>::Callback(Rc::new(|handle, args| {
            if let Some(recipient) = args.get(&String::from("recipient")) {
                let recipients = match recipient {
                    clitc::params::ParamValue::Array(vec) => vec.clone(),
                    _ => Vec::new(),
                };

                let mut context = handle.lock().unwrap();
                if let Some(mail) = &mut context.current_mail_writing {
                    mail.to(recipients);
                }
            }
        })));
        write.insert(String::from("cc"), Event::<ContextHandle, Emitter>::Callback(Rc::new(|handle, args| {
            if let Some(recipient) = args.get(&String::from("recipient")) {
                let recipients = match recipient {
                    clitc::params::ParamValue::Array(vec) => vec.clone(),
                    _ => Vec::new(),
                };

                let mut context = handle.lock().unwrap();
                if let Some(mail) = &mut context.current_mail_writing {
                    mail.cc(recipients);
                }
            }
        })));
        write.insert(String::from("bcc"), Event::<ContextHandle, Emitter>::Callback(Rc::new(|handle, args| {
            if let Some(recipient) = args.get(&String::from("recipient")) {
                let recipients = match recipient {
                    clitc::params::ParamValue::Array(vec) => vec.clone(),
                    _ => Vec::new(),
                };

                let mut context = handle.lock().unwrap();
                if let Some(mail) = &mut context.current_mail_writing {
                    mail.bcc(recipients);
                }
            }
        })));
        write.insert(String::from("subject"), Event::<ContextHandle, Emitter>::Callback(Rc::new(|handle, args| {
            if let Some(recipient) = args.get(&String::from("text")) {
                let text = match recipient {
                    clitc::params::ParamValue::Array(vec) => vec.join(" "),
                    _ => String::new(),
                };

                let mut context = handle.lock().unwrap();
                if let Some(mail) = &mut context.current_mail_writing {
                    mail.subject(text);
                }
            }
        })));
        write.insert(String::from("text"), Event::<ContextHandle, Emitter>::Callback(Rc::new(|handle, args| {
            use std::io::{stdin, stdout, Write};
            println!("Enter Text. Finish with '$'.");
            let mut lines = Vec::new();
            loop {
                print!("~ ");
                stdout().flush();
                let mut buf = String::new();
                stdin().read_line(&mut buf);
                buf = buf.trim().to_string();
                if buf.ends_with('$') {
                    lines.push((&buf[..buf.len() - 1]).to_string());
                    break;
                }
                lines.push(buf);
            }
            let content = lines.join("\n");

            let mut context = handle.lock().unwrap();
            if let Some(mail) = &mut context.current_mail_writing {
                mail.text(content);
            }
        })));
        write.insert(String::from("send"), Event::<ContextHandle, Emitter>::Callback(Rc::new(|handle, args| {
            // ToDo: Send functionality
            println!("send not yet implemented!");
        })));
        write.insert(String::from("save"), Event::<ContextHandle, Emitter>::Callback(Rc::new(|handle, args| {
            // ToDo: Save functionality
            println!("save not yet implemented!");
        })));
        write.insert(String::from("exit"), Event::<ContextHandle, Emitter>::Emit(Arc::clone(&handle), Rc::new(|_, emit_handle, args| {
            // Switch to mode global
            let mut emitter = emit_handle.lock().unwrap();
            *emitter = Some((Mode::Global, Some(GLOBAL_PROMPT.to_string())));
        })));
        write.insert(String::from("preview"), Event::<ContextHandle, Emitter>::Callback(Rc::new(|handle, _| {
            let mut context = handle.lock().unwrap();
            if let Some(mail) = &mut context.current_mail_writing {
                mail.show_preview();
            }
        })));
        states.insert(Mode::Write, write);
    }

    return (handle, states);
}

fn styling(code: u8) -> Style {
    match code {
        // Global prompt
        1 => Style::new().bold().yellow(),
        // Inbox Mode
        2 => Style::new().bold().green(),
        // Read Mode
        3 => Style::new().bold().cyan(),
        // Write Mode
        4 => Style::new().bold().magenta(),
        _ => Style::new(),
    }
}

fn input(prompt: String, code: u8) -> String {
    use std::io::{stdin, stdout, Write};
    let mut buf = String::new();
    print!("{} ", styling(code).apply_to(prompt));
    let _  = stdout().flush();
    stdin().read_line(&mut buf).expect("Could not read user input");
    buf = buf.trim().to_string();
    return buf;
}

fn main() {
    let cli_params = CliParameters::from_reader(File::open("D:/Dateien/tobias/data/cli-mail-rs/commands.json")
        .expect("Could not open command file"))
        .expect("Could not parse command file");
    let mut context = InboxManager::new(String::from("D:/Dateien/tobias/data/cli-mail-rs/accounts.yml"));
    match context.load_file() {
        Ok(_) => {},
        Err(e) => println!("Could not load account file! [{}]", e),
    };

    let mut event_handler = EventHandler::new(cli_params, WhitespaceSplitter, true, Arc::new(Mutex::new(context)));

    let mut cur_mode = Mode::Global;
    let mut prompt_path = Some(GLOBAL_PROMPT.to_string());

    let (handle, mut modes) = init_modes();
    let start_mode = modes.remove(&cur_mode).unwrap();
    event_handler.attach(start_mode);


    // User input loop
    loop {
        let prompt = cur_mode.get_prompt(prompt_path.clone());
        match event_handler.pass_command(input(prompt.0, prompt.1)) {
            Ok(_) => {},
            Err(e) => println!("{}", e),
        };

        {
            let mut mode_change = handle.lock().unwrap();
            // Check if mode change has been emitted
            if let Some((mode_ident, path)) = &*mode_change {
                // Check if that mode exists
                if let Some(next_mode) = modes.remove(&mode_ident) {
                    // Swap used modes & prompts, update current mode ident
                    let former_mode = event_handler.disattach();
                    event_handler.attach(next_mode);
                    modes.insert(cur_mode, former_mode);
                    cur_mode = mode_ident.clone();
                    prompt_path = path.clone();
                } else if *mode_ident == Mode::Exit {
                    break;
                }
                // Update emitter value
                *mode_change = None;
            }
        }
    }

    // handling exit
}
