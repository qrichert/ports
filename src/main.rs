use std::process::Command;

fn main() -> Result<(), ()> {
    //let status = Command::new("lsof").arg("-i", "-P", "-n"); // TODO: Manual grep
    let status = Command::new("/bin/bash")
        .arg("-c")
        .arg("lsof -i -P -n | grep --color=never LISTEN")
        .status();

    // TODO: improve this
    if let Ok(status) = status {
        if status.success() {
            return Ok(());
        }
    }
    Err(())
}
