use std::env;
use std::io::{self, BufRead};
use std::sync::{Arc, Mutex};

use socket_engine::endpoint::Endpoint;
use socket_engine::engine::Engine;
use socket_engine::event::EngineObserver;

struct Obs;

impl EngineObserver for Obs {
    fn notify(&mut self, event: socket_engine::event::SocketEngineEvent) {
        match event {
            socket_engine::event::SocketEngineEvent::Reception(items) => {
                        println!("< received: {:?}", items)
                    }
            socket_engine::event::SocketEngineEvent::Sent(uuid) => {
                 println!("> sent with uuid: {:?}", uuid)
            }
        }
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
        println!("will send {:?}", line);
        if let Err(err) = engine.send_async(distant_ep.clone(), text.into_bytes(), "some_id".to_string()) {
            eprintln!("failed to send message: {}", err);
        }
    }

    Ok(())
}
