use std::env;
use std::io::{self, BufRead};
use std::sync::{Arc, Mutex};

use dtchat_engine::encoding::{create_ack_proto_message, create_text_proto_message}; // adjust path as needed
use dtchat_engine::endpoint::Endpoint;
use dtchat_engine::engine::{Engine, EngineObserver};
use dtchat_engine::proto::ProtoMessage;

struct Obs;

impl EngineObserver for Obs {
    fn get_notification(&mut self, message: ProtoMessage) {
        println!("▶ received: {:?}", message);
    }
}

fn main() -> io::Result<()> {
    // --- 1) parse CLI argument
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} <local-endpoint> <distant-endpoint>", args[0]);
        std::process::exit(1);
    }

    let local_ep = match Endpoint::from_str(&args[1]) {
        Ok(ep) => ep,
        Err(e) => {
            eprintln!("Invalid local endpoint `{}`: {}", args[1], e);
            std::process::exit(1);
        }
    };
    let distant_ep = match Endpoint::from_str(&args[2]) {
        Ok(ep) => ep,
        Err(e) => {
            eprintln!("Invalid distant endpoint `{}`: {}", args[2], e);
            std::process::exit(1);
        }
    };

    // --- 2) create engine + observer
    let observer = Arc::new(Mutex::new(Obs));
    let engine = Engine::new(observer);
    engine.start_listener(local_ep);

    // --- 3) read lines from stdin
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let mut line = String::new();
    let mut alt = true;
    loop {
        println!("msg to send:");
        line.clear();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            // EOF
            break;
        }

        // strip trailing newline
        let text = line.trim_end().to_string();

        // --- 4) wrap in ProtoMessage + send
        let msg= if alt {create_text_proto_message(text)} else {create_ack_proto_message(text, true , true)};
        alt = !alt;
        println!("will send {:?}",msg);
        if let Err(err) = engine.send(distant_ep.clone(), msg) {
            eprintln!("failed to send message: {}", err);
        }
    }

    Ok(())
}
