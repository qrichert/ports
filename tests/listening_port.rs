use std::net::TcpListener;
use std::process::Command;

#[test]
fn lists_a_real_tcp_listener() {
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
