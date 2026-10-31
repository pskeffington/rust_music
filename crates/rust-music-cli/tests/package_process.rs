#![cfg(all(target_os = "macos", feature = "macos-output"))]

use rust_music_ipc::{
    read_frame, write_frame, Envelope, Playback, Reply, Request, PROTOCOL_VERSION,
};
use std::{
    io::Read,
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const PACKAGE_ID: &str = "org.rustboy.music";
const REPLY_DEADLINE: Duration = Duration::from_secs(3);

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn send_request(stdin: &mut ChildStdin, request_id: u64, body: Request) {
    let request = Envelope {
        package_id: PACKAGE_ID.into(),
        request_id,
        body,
    };
    write_frame(stdin, &request.encode().expect("encode request")).expect("write request to child");
}

fn start_reply_reader<R: Read + Send + 'static>(
    mut stdout: R,
) -> (
    Receiver<Result<Option<Vec<u8>>, rust_music_ipc::ProtocolError>>,
    JoinHandle<()>,
) {
    let (sender, receiver) = mpsc::channel();
    let reader = thread::spawn(move || loop {
        let frame = read_frame(&mut stdout);
        let finished = !matches!(frame, Ok(Some(_)));
        if sender.send(frame).is_err() || finished {
            break;
        }
    });
    (receiver, reader)
}

fn receive_reply(
    replies: &Receiver<Result<Option<Vec<u8>>, rust_music_ipc::ProtocolError>>,
) -> Envelope<Reply> {
    let payload = replies
        .recv_timeout(REPLY_DEADLINE)
        .expect("child reply deadline exceeded")
        .expect("valid child frame")
        .expect("child exited before replying");
    Envelope::<Reply>::decode(&payload).expect("decode child reply")
}

#[test]
fn spawned_cli_unit_handshakes_activates_inertly_and_shuts_down() {
    let child = Command::new(env!("CARGO_BIN_EXE_rust-music-cli"))
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn Rust Music process unit");
    let mut child = ChildGuard(child);
    let mut stdin = child.0.stdin.take().expect("child stdin");
    let stdout = child.0.stdout.take().expect("child stdout");
    let (replies, reply_reader) = start_reply_reader(stdout);

    send_request(
        &mut stdin,
        1,
        Request::Hello {
            min_version: PROTOCOL_VERSION,
            max_version: PROTOCOL_VERSION,
        },
    );
    let hello = receive_reply(&replies);
    assert_eq!(hello.package_id, PACKAGE_ID);
    assert_eq!(hello.request_id, 1);
    assert_eq!(
        hello.body,
        Reply::Hello {
            selected_version: PROTOCOL_VERSION
        }
    );

    send_request(
        &mut stdin,
        2,
        Request::Activate {
            width: 480,
            height: 320,
        },
    );
    let activate = receive_reply(&replies);
    assert_eq!(activate.package_id, PACKAGE_ID);
    assert_eq!(activate.request_id, 2);
    let Reply::View(view) = activate.body else {
        panic!("activation returns a view");
    };
    assert_eq!(view.playback, Playback::Empty);
    assert_eq!(view.track_count, 0);
    assert!(!view.audio_claimed);

    send_request(&mut stdin, 3, Request::Shutdown);
    let shutdown = receive_reply(&replies);
    assert_eq!(shutdown.package_id, PACKAGE_ID);
    assert_eq!(shutdown.request_id, 3);
    assert_eq!(shutdown.body, Reply::Ack);
    drop(stdin);

    let deadline = Instant::now() + REPLY_DEADLINE;
    let status = loop {
        if let Some(status) = child.0.try_wait().expect("poll child exit") {
            break status;
        }
        assert!(Instant::now() < deadline, "child exit deadline exceeded");
        thread::sleep(Duration::from_millis(10));
    };
    assert!(status.success(), "child exited with {status}");
    assert!(matches!(replies.recv_timeout(REPLY_DEADLINE), Ok(Ok(None))));
    reply_reader.join().expect("join child reply reader");
}

#[test]
fn spawned_cli_unit_fails_closed_on_wrong_package_identity() {
    let child = Command::new(env!("CARGO_BIN_EXE_rust-music-cli"))
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn Rust Music process unit");
    let mut child = ChildGuard(child);
    let mut stdin = child.0.stdin.take().expect("child stdin");
    let stdout = child.0.stdout.take().expect("child stdout");
    let (replies, reply_reader) = start_reply_reader(stdout);
    let request = Envelope {
        package_id: "org.rustboy.radio".into(),
        request_id: 1,
        body: Request::Hello {
            min_version: PROTOCOL_VERSION,
            max_version: PROTOCOL_VERSION,
        },
    };
    write_frame(
        &mut stdin,
        &request.encode().expect("encode wrong identity"),
    )
    .expect("write wrong identity to child");
    drop(stdin);

    assert!(matches!(
        replies
            .recv_timeout(REPLY_DEADLINE)
            .expect("child response deadline exceeded"),
        Ok(None)
    ));
    reply_reader.join().expect("join child reply reader");

    let deadline = Instant::now() + REPLY_DEADLINE;
    let status = loop {
        if let Some(status) = child.0.try_wait().expect("poll child exit") {
            break status;
        }
        assert!(Instant::now() < deadline, "child exit deadline exceeded");
        thread::sleep(Duration::from_millis(10));
    };
    assert!(
        !status.success(),
        "wrong package identity must fail the unit"
    );
}
