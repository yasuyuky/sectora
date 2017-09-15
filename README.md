# ghteam-auth

The program for ssh auth using github team.
Inspired by [Octopass](https://github.com/linyows/octopass).
Implemented with Rust.

## How to build

[![CircleCI](https://circleci.com/gh/yasuyuky/ghteam-auth.svg?style=svg)](https://circleci.com/gh/yasuyuky/ghteam-auth)

### On linux

```
cargo build --release
```

### Cross-compile on other platforms using docker

```
make
```

See Makefile for details

## How to install and setup

1. Copy executable and shared object to each paths
2. Place config file for this program.
3. Configure name service switch
4. Configure sshd
5. Configure PAM

### Copy executable file and shared object to each path

#### Copy executable file

Place `ghteam-auth` to `/usr/sbin/`.

#### Copy shared object

Place `libnss_ghteam.so` to `/usr/lib/`.

### Place config file for this program.

The minimal setting is like as follows.

```
token = "YOUR_PERSONAL_TOKEN_STRING"
org = "YOUR_ORGANIZATION"
[[team]]
name = "YOUR_TEAM1"
gid = YOUR_GID1
[[team]]
name = "YOUR_TEAM2"
gid = YOUR_GID1
group = "YOUR_GROUP_NAME"
```

See `struct Config` on `structs.rs` for details.

### Configure name service switch

Add the following lines to `/etc/nsswitch.conf`

```
passwd: files ghteam
shadow: files ghteam
group:  files ghteam
```

### Configure sshd

Add the following lines to `/etc/ssh/sshd_config`.

```
AuthorizedKeysCommandUser root
AuthorizedKeysCommand /usr/sbin/ghteam-auth key %u
UsePAM yes
```

#### In the case of old sshd

In the case of old sshd, you need to create the following shell script and put it in your PATH.

```ghteam-auth.sh
#!/bin/bash
/usr/sbin/ghteam-auth key $1
```

And sshd_config should look like this

```
AuthorizedKeysCommandUser root
AuthorizedKeysCommand /usr/sbin/ghteam-auth.sh
UsePAM yes
```

### Configure PAM

Add the following lines to `/etc/pam.d/sshd`.

```
auth requisite pam_exec.so quiet expose_authtok /usr/sbin/ghteam-auth pam
auth optional pam_unix.so not_set_pass use_first_pass nodelay
session required pam_mkhomedir.so skel: /etc/skel/ umask: 0022
```

And comment out the following line.

```
# @include common-auth
```

## Personal settings

To set personal settings, use `$HOME/.config/ghteam-auth.toml` like this.

```toml
sh = "/path/to/login/shell"
pass = "PASSWORD_HASH_STRING"
```

Use `mkpasswd` command to create your `PASSWORD_HASH_STRING`

```
mkpasswd -S $(head -c 4 /dev/urandom|xxd -p) -m sha-512
```


## LICENSE

MIT

## Special thanks

This program is deeply inspired by [Octopass](https://github.com/linyows/octopass).
Thank you.
