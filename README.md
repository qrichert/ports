# ports

![Crates.io License](https://img.shields.io/crates/l/ports)
![GitHub Tag](https://img.shields.io/github/v/tag/qrichert/ports?sort=semver&filter=*.*.*&label=release)
[![crates.io](https://img.shields.io/crates/d/ports?logo=rust&logoColor=white&color=orange)](https://crates.io/crates/ports)
[![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/qrichert/ports/ci.yml?label=tests)](https://github.com/qrichert/ports/actions)

_List listening ports._

It's sometimes hard to keep track of which process uses which port, or
what is running in the background.

`ports` is lightweight CLI sugar over commonly available system tools.
It combines and normalizes information from `lsof`, `ss`, and `ps`, then
presents it in one consistent view.

```console
$ ports 8000 50000-65535
COMMAND      PID  USER           HOST:PORT
rapportd     449  Quentin          *:61165
Python     22396  Quentin           *:8000
rustrover  30928  Quentin  127.0.0.1:63342
Transmiss  94671  Quentin          *:51413
```

<details><summary>With different levels of verbosity.</summary>
<p>

```console
$ ports -v 8000 50000-65535
COMMAND      PID  USER     TYPE        HOST:PORT  COMMAND
rapportd     449  Quentin  IPv4          *:61165  /usr/libexec/rapportd
rapportd     449  Quentin  IPv6          *:61165  /usr/libexec/rapportd
Python     22396  Quentin  IPv6           *:8000  /usr/local/Cellar/python@3.12/3.12.3/Frameworks/Python.framework/Versions/3.12/Resources/Python.app/Contents/MacOS/Python -m http.server
rustrover  30928  Quentin  IPv6  127.0.0.1:63342  /Applications/RustRover.app/Contents/MacOS/rustrover
Transmiss  94671  Quentin  IPv4          *:51413  /Applications/Transmission.app/Contents/MacOS/Transmission
Transmiss  94671  Quentin  IPv6          *:51413  /Applications/Transmission.app/Contents/MacOS/Transmission
```

```console
$ ports -vv 8000 50000-65535
COMMAND      PID  USER     TYPE  NODE        HOST:PORT  %CPU  %MEM    START       TIME  COMMAND
rapportd     449  Quentin  IPv4  TCP           *:61165   0.0   0.1  12Jul24    3:05.13  /usr/libexec/rapportd
rapportd     449  Quentin  IPv6  TCP           *:61165   0.0   0.1  12Jul24    3:05.13  /usr/libexec/rapportd
Python     22396  Quentin  IPv6  TCP            *:8000   0.0   0.1   5:47PM    0:00.18  /usr/local/Cellar/python@3.12/3.12.3/Frameworks/Python.framework/Versions/3.12/Resources/Python.app/Contents/MacOS/Python -m http.server
rustrover  30928  Quentin  IPv6  TCP   127.0.0.1:63342  18.3  32.2  Mon06PM  295:40.56  /Applications/RustRover.app/Contents/MacOS/rustrover
Transmiss  94671  Quentin  IPv4  TCP           *:51413   0.0   0.2   3Aug24   96:41.80  /Applications/Transmission.app/Contents/MacOS/Transmission
Transmiss  94671  Quentin  IPv6  TCP           *:51413   0.0   0.2   3Aug24   96:41.80  /Applications/Transmission.app/Contents/MacOS/Transmission
```

</p>
</details>

## Installation

Install from [crates.io] with Cargo:

```shell
cargo install ports
```

Pre-built binaries for Linux and macOS are available on the [latest
GitHub release].

[Documentation] is available on docs.rs.

[crates.io]: https://crates.io/crates/ports
[latest GitHub release]:
  https://github.com/qrichert/ports/releases/latest
[Documentation]: https://docs.rs/ports
