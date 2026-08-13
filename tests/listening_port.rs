use std::io;
use std::net::TcpListener;
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::sync::{Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

const CONDITION_WINDOW: Duration = Duration::from_millis(750);
const EXIT_TIMEOUT: Duration = Duration::from_secs(10);
const STATUS_CHECK_INTERVAL: Duration = Duration::from_millis(25);

static LISTENER_TEST_LOCK: Mutex<()> = Mutex::new(());

struct ChildGuard {
    child: Option<Child>,
}

impl ChildGuard {
    fn new(child: Child) -> Self {
        Self { child: Some(child) }
    }

    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.child
            .as_mut()
            .expect("child has already been collected")
            .try_wait()
    }

    fn kill(&mut self) -> io::Result<()> {
        self.child
            .as_mut()
            .expect("child has already been collected")
            .kill()
    }

    fn wait_with_output(&mut self) -> io::Result<Output> {
        self.child
            .take()
            .expect("child has already been collected")
            .wait_with_output()
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            _ = child.kill();
            _ = child.wait();
        }
    }
}

fn listener_test_lock() -> MutexGuard<'static, ()> {
    match LISTENER_TEST_LOCK.lock() {
        Ok(guard) => guard,
        Err(error) => error.into_inner(),
    }
}

fn spawn_wait(wait_option: &str, ports: &[u16]) -> ChildGuard {
    let child = Command::new(env!("CARGO_BIN_EXE_ports"))
        .arg(wait_option)
        .args(ports.iter().map(u16::to_string))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("cannot run ports");

    ChildGuard::new(child)
}

fn assert_running_for(child: &mut ChildGuard, duration: Duration) {
    let deadline = Instant::now() + duration;

    loop {
        if let Some(status) = child.try_wait().expect("cannot inspect ports process") {
            let output = child
                .wait_with_output()
                .expect("cannot collect ports output");
            panic!(
                "ports exited unexpectedly with {status}:\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            );
        }

        if Instant::now() >= deadline {
            return;
        }

        thread::sleep(STATUS_CHECK_INTERVAL);
    }
}

fn wait_for_output(child: &mut ChildGuard, timeout: Duration) -> Output {
    let deadline = Instant::now() + timeout;

    loop {
        if child
            .try_wait()
            .expect("cannot inspect ports process")
            .is_some()
        {
            return child
                .wait_with_output()
                .expect("cannot collect ports output");
        }

        if Instant::now() >= deadline {
            let kill_error = child.kill().err();
            let output = child
                .wait_with_output()
                .expect("cannot collect timed-out ports output");
            panic!(
                "ports did not exit within {timeout:?} (kill error: {kill_error:?}):\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            );
        }

        thread::sleep(STATUS_CHECK_INTERVAL);
    }
}

#[test]
fn lists_a_real_tcp_listener() {
    let _listener_test_guard = listener_test_lock();
    let listener = TcpListener::bind("127.0.0.1:0").expect("cannot bind test listener");
    let port = listener
        .local_addr()
        .expect("test listener has no local address")
        .port();

    let output = Command::new(env!("CARGO_BIN_EXE_ports"))
        .arg(port.to_string())
        .output()
        .expect("cannot run ports");

    assert!(
        output.status.success(),
        "ports failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains(&format!(":{port}")),
        "ports did not list the test listener:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn wait_for_none_waits_for_a_listener_to_close() {
    let _listener_test_guard = listener_test_lock();
    let listener = TcpListener::bind("127.0.0.1:0").expect("cannot bind test listener");
    let port = listener
        .local_addr()
        .expect("test listener has no local address")
        .port();

    let mut child = spawn_wait("--wait-for-none", &[port]);
    assert_running_for(&mut child, CONDITION_WINDOW);
    drop(listener);

    let output = wait_for_output(&mut child, EXIT_TIMEOUT);

    assert!(
        output.status.success(),
        "ports failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    assert!(
        output.stdout.is_empty(),
        "ports produced unexpected output:\n{}",
        String::from_utf8_lossy(&output.stdout),
    );
}

#[test]
fn wait_for_some_waits_for_a_listener() {
    let _listener_test_guard = listener_test_lock();
    let reservation = TcpListener::bind("127.0.0.1:0").expect("cannot reserve test port");
    let port = reservation
        .local_addr()
        .expect("test port reservation has no local address")
        .port();
    // Releasing the reservation creates the initial false condition.
    // This leaves a small OS-wide port-reuse race. The module lock only
    // prevents this file's tests from competing.
    drop(reservation);

    let mut child = spawn_wait("--wait-for-some", &[port]);
    assert_running_for(&mut child, CONDITION_WINDOW);

    let _listener = TcpListener::bind(("127.0.0.1", port)).expect("cannot bind test listener");
    let output = wait_for_output(&mut child, EXIT_TIMEOUT);

    assert!(
        output.status.success(),
        "ports failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains(&format!(":{port}")),
        "ports did not list the test listener:\n{}",
        String::from_utf8_lossy(&output.stdout),
    );
}

#[test]
fn wait_for_all_waits_for_every_listener() {
    let _listener_test_guard = listener_test_lock();
    let first_reservation =
        TcpListener::bind("127.0.0.1:0").expect("cannot reserve first test port");
    let second_reservation =
        TcpListener::bind("127.0.0.1:0").expect("cannot reserve second test port");
    let first_port = first_reservation
        .local_addr()
        .expect("first test port reservation has no local address")
        .port();
    let second_port = second_reservation
        .local_addr()
        .expect("second test port reservation has no local address")
        .port();
    // Releasing the reservations creates the initial false condition.
    // This leaves a small OS-wide port-reuse race. The module lock only
    // prevents this file's tests from competing.
    drop(first_reservation);
    drop(second_reservation);

    let mut child = spawn_wait("--wait-for-all", &[first_port, second_port]);
    assert_running_for(&mut child, CONDITION_WINDOW);

    let _first_listener =
        TcpListener::bind(("127.0.0.1", first_port)).expect("cannot bind first test listener");
    assert_running_for(&mut child, CONDITION_WINDOW);

    let _second_listener =
        TcpListener::bind(("127.0.0.1", second_port)).expect("cannot bind second test listener");
    let output = wait_for_output(&mut child, EXIT_TIMEOUT);

    assert!(
        output.status.success(),
        "ports failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains(&format!(":{first_port}")),
        "ports did not list the first test listener:\n{}",
        String::from_utf8_lossy(&output.stdout),
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains(&format!(":{second_port}")),
        "ports did not list the second test listener:\n{}",
        String::from_utf8_lossy(&output.stdout),
    );
}
