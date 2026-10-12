use super::*;
use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::Command;
use std::thread;

fn isolated(name: &str, test: impl FnOnce(&std::path::Path)) {
    const CHILD: &str = "DEFLORTA_DISCORD_TEST_CHILD";
    if let Some(directory) = std::env::var_os(CHILD) {
        test(std::path::Path::new(&directory));
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture"])
        .env(CHILD, directory.path())
        .env("XDG_RUNTIME_DIR", directory.path())
        .env("TMPDIR", directory.path())
        .env_remove("SNAP")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn accept(listener: &UnixListener, id: &str) -> UnixStream {
    let mut stream = accept_socket(listener);
    let (opcode, handshake) = read(&mut stream);
    assert_eq!(opcode, 0);
    assert_eq!(handshake["v"], 1);
    assert_eq!(handshake["client_id"], id);
    send(
        &mut stream,
        1,
        &json!({"cmd":"DISPATCH", "evt":"READY", "data":{}}),
    );
    stream
}

fn accept_socket(listener: &UnixListener) -> UnixStream {
    listener.set_nonblocking(true).unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(25);
    let stream = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(
                    std::time::Instant::now() < deadline,
                    "Discord connection timed out"
                );
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("{error}"),
        }
    };
    stream
        .set_read_timeout(Some(Duration::from_secs(25)))
        .unwrap();
    stream.set_write_timeout(Some(IO_TIMEOUT)).unwrap();
    stream
}

fn read(stream: &mut UnixStream) -> (u32, Value) {
    let mut header = [0_u8; 8];
    stream.read_exact(&mut header).unwrap();
    let opcode = u32::from_le_bytes(header[..4].try_into().unwrap());
    let size = u32::from_le_bytes(header[4..].try_into().unwrap());
    assert!(size <= 4096);
    let mut bytes = vec![0; size as usize];
    stream.read_exact(&mut bytes).unwrap();
    (opcode, serde_json::from_slice(&bytes).unwrap())
}

fn send(stream: &mut UnixStream, opcode: u32, payload: &Value) {
    let bytes = serde_json::to_vec(payload).unwrap();
    let mut frame = Vec::new();
    frame.extend_from_slice(&opcode.to_le_bytes());
    frame.extend_from_slice(&u32::try_from(bytes.len()).unwrap().to_le_bytes());
    frame.extend_from_slice(&bytes);
    stream.write_all(&frame).unwrap();
}

fn activity(stream: &mut UnixStream) -> Value {
    let (opcode, request) = read(stream);
    assert_eq!(opcode, 1);
    assert_eq!(request["cmd"], "SET_ACTIVITY");
    assert_eq!(request["args"]["pid"], std::process::id());
    send(
        stream,
        1,
        &json!({"cmd":"SET_ACTIVITY", "nonce":request["nonce"], "data":{}}),
    );
    request["args"]["activity"].clone()
}

#[test]
fn ipc_coalesces_updates_clears_reconnects_and_shuts_down() {
    isolated(
        "discord::tests::ipc_coalesces_updates_clears_reconnects_and_shuts_down",
        |directory| {
            let listener = UnixListener::bind(directory.join("discord-ipc-0")).unwrap();
            let mut discord = Discord::default();
            let config = DiscordConfig {
                application_id: "1234567890123456789".into(),
            };
            let mut desired = DiscordActivity {
                details: Some("Chapter one".into()),
                state: Some("Reading".into()),
                large_image: Some("cover".into()),
                large_text: Some("The story".into()),
                small_image: Some("character".into()),
                small_text: Some("A friend".into()),
                start_timestamp: Some(1_700_000_000),
            };
            discord.sync(Some(&config), Some(&desired));
            let mut stream = accept(&listener, &config.application_id);
            let (opcode, request) = read(&mut stream);
            assert_eq!(opcode, 1);
            let payload = &request["args"]["activity"];
            assert_eq!(payload["details"], "Chapter one");
            assert_eq!(payload["state"], "Reading");
            assert_eq!(payload["assets"]["large_image"], "cover");
            assert_eq!(payload["assets"]["small_text"], "A friend");
            assert_eq!(payload["timestamps"]["start"], 1_700_000_000);
            send(&mut stream, 3, &json!({"ping":true}));
            assert_eq!(read(&mut stream), (4, json!({"ping":true})));
            send(
                &mut stream,
                1,
                &json!({"cmd":"SET_ACTIVITY", "nonce":"unrelated"}),
            );
            send(
                &mut stream,
                1,
                &json!({"cmd":"SET_ACTIVITY", "nonce":request["nonce"]}),
            );

            desired.details = Some("Intermediate".into());
            discord.sync(Some(&config), Some(&desired));
            desired.details = Some("Latest chapter".into());
            discord.sync(Some(&config), Some(&desired));
            assert_eq!(activity(&mut stream)["details"], "Latest chapter");

            discord.sync(Some(&config), None);
            assert_eq!(activity(&mut stream), Value::Null);
            discord.sync(None, None);
            assert_eq!(activity(&mut stream), Value::Null);
            let mut byte = [0_u8];
            assert_eq!(stream.read(&mut byte).unwrap(), 0);

            let config = DiscordConfig {
                application_id: "987654321098765432".into(),
            };
            discord.sync(Some(&config), Some(&desired));
            let mut stream = accept(&listener, &config.application_id);
            let (_, rejected) = read(&mut stream);
            send(
                &mut stream,
                1,
                &json!({
                    "cmd":"SET_ACTIVITY", "evt":"ERROR", "nonce":rejected["nonce"],
                    "data":{"code":4000,"message":"test rejection"},
                }),
            );
            drop(stream);
            // A rejected activity must retry instead of being marked sent.
            let mut stream = accept(&listener, &config.application_id);
            assert_eq!(activity(&mut stream)["details"], "Latest chapter");
            drop(stream);
            // A heartbeat discovers the closed connection; retry restores the
            // latest desired activity without needing another game event.
            let mut stream = accept(&listener, &config.application_id);
            assert_eq!(activity(&mut stream)["details"], "Latest chapter");
            let shutdown = thread::spawn(move || discord.shutdown());
            assert_eq!(activity(&mut stream), Value::Null);
            shutdown.join().unwrap();
            assert_eq!(stream.read(&mut byte).unwrap(), 0);
        },
    );
}

#[test]
fn unresponsive_handshake_does_not_block_the_game_or_shutdown() {
    isolated(
        "discord::tests::unresponsive_handshake_does_not_block_the_game_or_shutdown",
        |directory| {
            let listener = UnixListener::bind(directory.join("discord-ipc-0")).unwrap();
            let mut discord = Discord::default();
            let config = DiscordConfig {
                application_id: "1234567890123456789".into(),
            };
            let started = std::time::Instant::now();
            discord.sync(Some(&config), Some(&DiscordActivity::default()));
            assert!(started.elapsed() < Duration::from_millis(250));
            let mut stream = accept_socket(&listener);
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            assert_eq!(read(&mut stream).0, 0);
            let stopped = std::time::Instant::now();
            discord.shutdown();
            assert!(stopped.elapsed() < Duration::from_secs(2));
            assert_eq!(stream.read(&mut [0]).unwrap(), 0);
        },
    );
}
