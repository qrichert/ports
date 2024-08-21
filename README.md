# ports

[![license: GPL v3+](https://img.shields.io/badge/license-GPLv3+-blue)](https://www.gnu.org/licenses/gpl-3.0)
![GitHub Tag](https://img.shields.io/github/v/tag/qrichert/ports?sort=semver&filter=*.*.*&label=release)
[![crates.io](https://img.shields.io/crates/d/ports?logo=rust&logoColor=white&color=orange)](https://crates.io/crates/ports)

_List listening ports._

It's sometimes hard to keep track of which process uses which port, or
what is running in the background.

```console
$ ports 8000 50000-65535
COMMAND      PID  USER     TYPE  NODE        HOST:PORT
rapportd     449  Quentin  IPv4  TCP           *:61165
rapportd     449  Quentin  IPv6  TCP           *:61165
Python     22396  Quentin  IPv6  TCP            *:8000
rustrover  30928  Quentin  IPv6  TCP   127.0.0.1:63342
Transmiss  94671  Quentin  IPv4  TCP           *:51413
Transmiss  94671  Quentin  IPv6  TCP           *:51413
```

<details><summary>With different levels of verbosity.</summary>
<p>

```console
$ ports -vv 8000 50000-65535
COMMAND      PID  USER     TYPE  NODE        HOST:PORT  COMMAND
rapportd     449  Quentin  IPv4  TCP           *:61165  /usr/libexec/rapportd
rapportd     449  Quentin  IPv6  TCP           *:61165  /usr/libexec/rapportd
Python     22396  Quentin  IPv6  TCP            *:8000  /usr/local/Cellar/python@3.12/3.12.3/Frameworks/Python.framework/Versions/3.12/Resources/Python.app/Contents/MacOS/Python -m http.server
rustrover  30928  Quentin  IPv6  TCP   127.0.0.1:63342  /Applications/RustRover.app/Contents/MacOS/rustrover
Transmiss  94671  Quentin  IPv4  TCP           *:51413  /Applications/Transmission.app/Contents/MacOS/Transmission
Transmiss  94671  Quentin  IPv6  TCP           *:51413  /Applications/Transmission.app/Contents/MacOS/Transmission
```

```console
$ ports -vvv 8000 50000-65535
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

### Directly

```console
$ wget https://github.com/qrichert/ports/releases/download/X.X.X/ports-X.X.X-xxx
$ sudo install ./ports-* /usr/local/bin/ports
$ sudo ln -s /usr/local/bin/ports /usr/local/bin/cr
```

### Manual Build

#### System-wide

```console
$ git clone https://github.com/qrichert/ports.git
$ cd ports
$ make build
$ sudo make install
```

#### Through Cargo

```shell
cargo install ports
cargo install --git https://github.com/qrichert/ports.git
```
