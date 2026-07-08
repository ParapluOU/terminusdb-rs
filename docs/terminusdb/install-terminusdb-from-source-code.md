---
tags:
  - how-to
  - installation
  - curl
  - self-hosted
  - advanced
title: Install TerminusDB from Source Code
nextjs:
  metadata:
    title: Install TerminusDB from Source Code
    description: Everything you need to install TerminusDB from source code.
    keywords: terminusdb, getting started, install, install terminusdb from source code, setup
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/install-terminusdb-from-source-code/
media: []
---

{% callout type="note" %}
**Prerequisites**
- SWI-Prolog installed (version 8.4+)
- Rust toolchain installed
- Git installed
- A Unix-like environment (Linux or macOS)
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will have TerminusDB compiled from source and running on your machine.
{% /callout %}

## Install steps

Install, build and run TerminusDB from source code with the following steps.

- [Install steps](#install-steps)
- [Install SWI-Prolog, Rust and clang](#install-swi-prolog-rust-and-clang)
  - [Linux](#linux)
  - [macOS](#macos)
- [Clone the TerminusDB repository](#clone-the-terminusdb-repository)
- [Make the TerminusDB Command Line Interface](#make-the-terminusdb-command-line-interface)
  - [Linux](#linux-1)
  - [macOS](#macos-1)
- [Run the TerminusDB system database](#run-the-terminusdb-system-database)
  - [Linux](#linux-2)
  - [macOS](#macos-2)
- [Further Reading](#further-reading)

> **Install from source code on Windows:**  
>   
> Install [WSL](https://ubuntu.com/wsl) and [Ubuntu](https://ubuntu.com/#download)  
>   
> In Ubuntu terminal: `sudo apt install make libgmp-dev`  
>   
> In Ubuntu terminal: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`  
>   
> Follow the install steps below for **Debian or Ubuntu**

## Install SWI-Prolog, Rust and clang

### Linux

Install [Rust](https://www.rust-lang.org/tools/install) by following the Rust installation guide.

* * *

**Arch Linux**

Install all dependencies of all the required libraries using [sudo](https://www.sudo.ws/download.html) and [git](https://git-scm.com/downloads).

```bash
sudo pacman -S git swi-prolog make automake autoconf libtool zlib pkgconf gcc clang gmp
```

* * *

**Debian or Ubuntu**

Install using the apt package manager.

```bash
cat /etc/*release | grep ubuntu > /dev/null && (sudo apt-get install software-properties-common; sudo apt-add-repository ppa:swi-prolog/stable)
sudo apt-get update
sudo apt install swi-prolog clang libgmp-dev
```

* * *

**Fedora or Red Hat**

Install using [sudo](https://www.sudo.ws/download.html).

```bash
sudo dnf install pl pl-devel clang gmp-devel
```

### macOS

Install `swi-prolog` and `rust` using [homebrew](https://brew.sh).

```bash
brew install gmp
brew install swi-prolog
brew install rust
```

## Clone the TerminusDB repository

Identical for all operating systems: Clone the `terminusdb` repository from GitHub.

```bash
git clone https://github.com/terminusdb/terminusdb
```

## Make the TerminusDB Command Line Interface

`make` the `terminusdb` [Command Line Interface (CLI)](/docs/terminusdb-cli-commands/) binary.

### Linux

```bash
cd terminusdb
make install-tus
make
```

### macOS

```bash
cd terminusdb
make install-tus
make
```

## Run the TerminusDB system database

### Linux

Initialize the system database and choose a password for the admin user.

*   Server starts on `http://127.0.0.1:6363`

```bash
./terminusdb store init --key "my_password_here"
./terminusdb serve
```

### macOS

*   Initialize the system database.
*   Server starts on `http://127.0.0.1:6363`

```bash
./terminusdb store init --key root
./terminusdb serve
```

## Further Reading

[**The TerminusDB Command Line Interface**](/docs/terminusdb-cli-commands/)