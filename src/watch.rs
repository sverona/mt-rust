use notify_debouncer_mini::{new_debouncer, notify::RecursiveMode, DebounceEventResult};

use std::error::Error;
use std::io;
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub fn watch() -> Result<(), Box<dyn Error>> {
    // cf. https://doc.rust-lang.org/std/net/struct.TcpListener.html
    // Start a TCP server. Let the system assign a port.
    // This returns an io::Result, which should tell you what sort of
    // things could possibly go wrong.
    let websocket_server = TcpListener::bind("localhost:0")?;

    // Figure out what port we're on.
    let websocket_port = websocket_server.local_addr()?.port();

    crate::build(Some(websocket_port))?;

    // https://docs.rs/crossbeam-channel/latest/crossbeam_channel/
    // TODO Try to replace this with stdlib std::sync::mpsc for fun!
    // Obtain a transmitter and receiver for `notify`.
    // TODO Turbofish???
    let (notify_tx, notify_rx) = crossbeam_channel::unbounded::<DebounceEventResult>();

    // https://docs.rs/notify-debouncer-mini/latest/notify_debouncer_mini/
    // This one can't be replaced, it goes with `notify`.
    let mut debouncer = new_debouncer(Duration::from_millis(500), notify_tx)?;

    let (build_tx, build_rx) = crossbeam_channel::unbounded::<()>();

    // https://doc.rust-lang.org/std/thread/fn.spawn.html
    // TODO move??
    let build_thread = thread::spawn(move || {
        notify_rx
            .iter()
            .filter_map(|rx| match rx {
                Ok(_) => Some(()),
                Err(e) => {
                    eprintln!("Error: {:?}", e);
                    None
                }
            })
            .map(|_| {
                // Run the command `cargo run build --websocket-port PORT`.
                Command::new("cargo")
                    .arg("run")
                    .arg("build")
                    .args(["--websocket-port", &websocket_port.to_string()])
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit())
                    .spawn()
                    .expect("cargo failed to start")
                    .wait()
                    .expect("cargo error")
            })
            // Log nonzero exit statuses.
            .try_for_each(|exit_status| {
                if exit_status.success() {
                    build_tx.send(())
                } else {
                    eprintln!("Error: Got status {:?}", exit_status);
                    Ok(())
                }
            })
    });

    // https://docs.rs/tungstenite/latest/tungstenite/
    // TODO What is all this doing???
    let sockets: Arc<Mutex<Vec<tungstenite::WebSocket<TcpStream>>>> =
        Arc::new(Mutex::new(Vec::new()));
    let sockets_clone = sockets.clone();

    let websocket_thread = thread::spawn(move || {
        websocket_server.incoming().for_each(|stream| {
            // TODO UNWRAPS
            let websocket = tungstenite::accept(stream.unwrap()).unwrap();
            sockets.lock().unwrap().push(websocket);
        });
    });

    let reload_thread = thread::spawn(move || {
        build_rx.iter().for_each(|_| {
            let mut sockets = sockets_clone.lock().unwrap();

            // TODO Make this stateless
            let mut broken = vec![];

            sockets.iter_mut().enumerate().for_each(|(index, socket)| {
                if let Some(err) = socket.send("reload".into()).err() {
                    match err {
                        tungstenite::error::Error::Io(e) => {
                            if e.kind() == io::ErrorKind::BrokenPipe {
                                broken.push(index);
                            }
                        }
                        e => {
                            eprintln!("Error: {:?}", e);
                        }
                    };
                };
            });

            broken.into_iter().rev().for_each(|index| {
                sockets.remove(index);
            });
        })
    });

    debouncer
        .watcher()
        .watch(Path::new("./content"), RecursiveMode::Recursive)?;
    debouncer
        .watcher()
        .watch(Path::new("./static"), RecursiveMode::Recursive)?;
    debouncer
        .watcher()
        .watch(Path::new("./src"), RecursiveMode::Recursive)?;

    let dist = std::env::current_dir()?.join("dist");
    let server = file_serve::ServerBuilder::new(dist)
        .hostname("0.0.0.0")
        .port(1729)
        .build();

    println!("Running on http://{}", server.addr());
    println!("Hit Ctrl-C to stop");
    server.serve()?;

    build_thread.join().unwrap()?;
    reload_thread.join().unwrap();
    websocket_thread.join().unwrap();

    Ok(())
}
