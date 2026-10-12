//! Desktop Rich Presence. IPC stays on one worker and all I/O has a deadline.

use std::sync::mpsc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use deflorta_common::discord::{DiscordActivity, DiscordConfig};
use deflorta_common::worker::runtime;
use log::debug;
use presenceforge::{ActivityBuilder, AsyncDiscordIpcClient, Opcode};
use serde_json::{Value, json};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::{Instant, timeout};

const IO_TIMEOUT: Duration = Duration::from_secs(1);
const RETRY_DELAY: Duration = Duration::from_secs(5);
const UPDATE_INTERVAL: Duration = Duration::from_secs(15);

#[derive(Clone, Default, PartialEq, Eq)]
struct Desired {
    config: Option<DiscordConfig>,
    activity: Option<DiscordActivity>,
}

#[derive(Default)]
pub struct Discord {
    desired: Desired,
    sender: Option<watch::Sender<Desired>>,
    finished: Option<mpsc::Receiver<()>>,
    task: Option<JoinHandle<()>>,
}

impl Discord {
    pub fn sync(&mut self, config: Option<&DiscordConfig>, activity: Option<&DiscordActivity>) {
        if self.desired.config.as_ref() == config && self.desired.activity.as_ref() == activity {
            return;
        }
        self.desired = Desired {
            config: config.cloned(),
            activity: activity.cloned(),
        };
        if let Some(sender) = &self.sender {
            sender.send_replace(self.desired.clone());
        } else if config.is_some() {
            self.start();
        }
    }

    fn start(&mut self) {
        let (sender, receiver) = watch::channel(self.desired.clone());
        let (done, finished) = mpsc::channel();
        self.task = Some(runtime().spawn(async move {
            run(receiver).await;
            let _ = done.send(());
        }));
        self.sender = Some(sender);
        self.finished = Some(finished);
    }

    pub fn shutdown(&mut self) {
        // Closing the watch channel wakes an idle worker immediately. Even a
        // stalled handshake plus final clear cannot delay exit indefinitely.
        self.sender.take();
        if let Some(task) = self.task.take()
            && let Some(finished) = self.finished.take()
            && finished
                .recv_timeout(IO_TIMEOUT * 2 + Duration::from_millis(500))
                .is_err()
        {
            task.abort();
        }
    }
}

impl Drop for Discord {
    fn drop(&mut self) {
        self.shutdown();
    }
}

async fn run(mut receiver: watch::Receiver<Desired>) {
    let mut desired = receiver.borrow_and_update().clone();
    let mut client: Option<AsyncDiscordIpcClient> = None;
    let mut due = Instant::now();
    let mut nonce = 0_u64;
    loop {
        if Instant::now() >= due {
            if let Some(config) = &desired.config {
                let result = timeout(IO_TIMEOUT, async {
                    if client.is_none() {
                        let mut connection =
                            AsyncDiscordIpcClient::new(&config.application_id).await?;
                        connection.connect().await?;
                        client = Some(connection);
                    }
                    exchange(
                        client.as_mut().unwrap(),
                        desired.activity.as_ref(),
                        &mut nonce,
                    )
                    .await
                })
                .await;
                if matches!(result, Ok(Ok(()))) {
                    due = Instant::now() + UPDATE_INTERVAL;
                } else {
                    debug!("Discord presence unavailable: {result:?}");
                    // A timed-out read may have consumed part of a frame. Always
                    // drop that connection before retrying the latest activity.
                    client = None;
                    due = Instant::now() + RETRY_DELAY;
                }
            } else {
                due = Instant::now() + UPDATE_INTERVAL;
            }
        }
        tokio::select! {
            changed = receiver.changed() => {
                if changed.is_err() { break; }
                let next = receiver.borrow_and_update().clone();
                if next.config != desired.config {
                    clear(&mut client, &mut nonce).await;
                    due = Instant::now();
                } else if next.activity.is_none() {
                    // Clearing is immediate, even when ordinary updates are
                    // being coalesced to respect Discord's activity rate limit.
                    due = Instant::now();
                }
                desired = next;
            }
            () = tokio::time::sleep_until(due) => {}
        }
    }
    clear(&mut client, &mut nonce).await;
}

async fn clear(client: &mut Option<AsyncDiscordIpcClient>, nonce: &mut u64) {
    if let Some(mut connection) = client.take() {
        let _ = timeout(IO_TIMEOUT, exchange(&mut connection, None, nonce)).await;
    }
}

fn activity_value(activity: &DiscordActivity) -> Result<Value> {
    let mut builder = ActivityBuilder::new();
    if let Some(value) = &activity.details {
        builder = builder.details(value);
    }
    if let Some(value) = &activity.state {
        builder = builder.state(value);
    }
    if let Some(value) = &activity.large_image {
        builder = builder.large_image(value);
    }
    if let Some(value) = &activity.large_text {
        builder = builder.large_text(value);
    }
    if let Some(value) = &activity.small_image {
        builder = builder.small_image(value);
    }
    if let Some(value) = &activity.small_text {
        builder = builder.small_text(value);
    }
    if let Some(value) = activity.start_timestamp {
        builder = builder.start_timestamp(value);
    }
    Ok(serde_json::to_value(builder.build())?)
}

async fn exchange(
    client: &mut AsyncDiscordIpcClient,
    activity: Option<&DiscordActivity>,
    nonce: &mut u64,
) -> Result<()> {
    *nonce = nonce.wrapping_add(1);
    let request_nonce = nonce.to_string();
    client
        .send_message(
            Opcode::Frame,
            &json!({
                "cmd": "SET_ACTIVITY",
                "args": {
                    "pid": std::process::id(),
                    "activity": activity.map(activity_value).transpose()?,
                },
                "nonce": request_nonce,
            }),
        )
        .await?;
    // Use the library's framing/discovery/transport with explicit RPC response
    // checks: Discord errors are evt=ERROR with data, and pings need a pong.
    loop {
        let (opcode, response) = client.recv_message().await?;
        if opcode == Opcode::Ping {
            client.send_message(Opcode::Pong, &response).await?;
            continue;
        }
        if opcode == Opcode::Close {
            bail!("Discord closed the IPC connection: {response}");
        }
        if opcode != Opcode::Frame {
            continue;
        }
        if response.get("nonce").and_then(Value::as_str) != Some(&request_nonce) {
            continue;
        }
        if response.get("evt").and_then(Value::as_str) == Some("ERROR") {
            bail!("Discord rejected the activity: {}", response["data"]);
        }
        if let Some(error) = response.get("error") {
            bail!("Discord rejected the activity: {error}");
        }
        response
            .get("cmd")
            .and_then(Value::as_str)
            .filter(|command| *command == "SET_ACTIVITY")
            .context("unexpected Discord activity response")?;
        return Ok(());
    }
}

#[cfg(all(test, unix))]
#[path = "discord/tests.rs"]
mod tests;
