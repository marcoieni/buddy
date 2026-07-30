# buddy

> [!NOTE]
> This project is meant for people working on the Rust Project infrastructure.

Run codex in a Linux VM using [lima](https://lima-vm.io/).
Control it from [ChatGPT desktop](https://chatgpt.com/download/).

The VM has read-only access to Cloud services like DataDog and Fastly.
Credentials are read automatically from the Rust Foundation 1Password.

## Set up

### 1. Install host tools

- Install the [Rust toolchain](https://rustup.rs/).

- Install the required tools on the host. E.g. on MacOS:

  ```sh
  brew install lima just
  ```

- Set up one password:
  1. Check if you have the 1Password CLI installed:

     ```sh
     op --version
     ```

  2. If not, install it. E.g. on MacOS:

     ```sh
     brew install 1password-cli
     ```

  3. If you have the 1password desktop app installed, enable:
     **Settings > Developer > Integrate with 1password CLI**.

### 2. Set up the VM

- Create the VM:

  ```sh
  just create
  ```

- Make sure you can access the 1Password `Infrastructure` vault in the Rust-Foundation
  1Password. It contains the read-only credentials for Datadog and Fastly. If
  you are experiencing issues, check the
  [authentication docs](./docs/authentication.md).

- Enable "Device code authorization for Codex" in
  [ChatGPT security settings](https://chatgpt.com/#settings/Security).

- Login to Codex, Datadog and Fastly from the guest:

  ```sh
  just login
  ```

### 3. Set up projects

The host directory `~/buddy` is mounted inside the VM at `$HOME/work`.
Place your projects in the host directory:
`~/buddy/<project>` on the host will be available as `~/work/<project>` in the guest.

### 4. Set up SSH

- Expose Lima's generated SSH configuration to OpenSSH:
  - Add this line to `~/.ssh/config`, outside any `Host` block (e.g. at the beginning of the file):

    ```
    Include ~/.lima/buddy/ssh.config
    ```

  - (alternative) to expose every Lima VM's SSH configuration, add this line instead:

    ```
    Include ~/.lima/*/ssh.config
    ```

- Test the SSH connection to the VM:

  ```sh
  ssh lima-buddy
  ```

- (Optional) Launch the `codex` command inside the VM to start the Codex TUI.

See the [Lima SSH documentation](https://lima-vm.io/docs/usage/ssh/) for more details.

### 5. Connect from ChatGPT desktop (optional)

- Install [ChatGPT desktop](https://chatgpt.com/download/)

- Configure the connection in ChatGPT desktop:

  1. Open **Settings > Connections > SSH**.
  2. Select **Add**, then select `lima-buddy`.
  3. Choose `~/work` as the remote project folder, or `~/work/<project>` for a
     specific repository.
  4. Select `lima-buddy` as the run location when starting a task, customizing
     the permission, model, etc:

     ![ChatGPT desktop SSH connection settings](chatgpt-task.png)

See the [ChatGPT remote connections documentation](https://learn.chatgpt.com/docs/remote-connections#connect-to-an-ssh-host)
for more details.

## Useful commands

- Check the [justfile](./justfile) for other available commands.

- Run commands in the VM:

  ```sh
  limactl shell buddy uname -a
  ```

## Security

- Read-only cloud tokens are still secrets and can be copied or abused. This
  is why we should use tokens with an expiration date.
  - Rotate tokens before they expire, and revoke a token immediately if the VM
    or token might have been compromised.
- Keep credentials out of ordinary cloud configuration. For example, Fastly `global:read`
  correctly treats VCL as readable configuration, so a credential embedded in
  VCL is exposed even though protected secret fields remain unavailable.
- Only use State of the Art (SOTA) models. Weak models can be tricked into
  revealing secrets more easily.

### Internet access

Buddy does not restrict Codex's internet access by default. An experimental,
opt-in [`requirements.toml`](docs/codex/requirements.toml) is available for
evaluation. It defines these profiles:

- `:workspace` is the default profile. It can edit mounted projects but cannot
  make network requests from commands.
- `buddy-observability` adds command-line network access to Datadog and Fastly
  only. Select it for tasks that need those services.
- `:read-only` remains available, but `:danger-full-access` is not allowed.
- Web search is limited to cached results rather than live browsing.

There is no general-purpose "read-only internet" security boundary. Blocking
HTTP write methods would not prevent data exfiltration: a secret can be placed
in a URL, header, DNS query, or other nominally read-only request. The
`buddy-observability` profile therefore restricts destinations rather than HTTP
methods. Datadog and Fastly can still receive data sent to their allowed
domains, so use the offline `:workspace` profile unless a task needs them.

### Passwordless sudo

The template explicitly sets `user.passwordlessSudo: true`, which is
[Lima's default for Linux guests](https://lima-vm.io/docs/config/sudo/).
This gives Codex unrestricted root access inside the guest, but does not grant
root access on the host. The VM boundary and the resources exposed to the VM,
such as the writable `$HOME/work` mount, remain the main security boundary.

Enabling passwordless sudo has this advantage:

- Codex and other automation can install packages, update the guest, and fix
  system configuration without waiting for a password prompt.

It also has these disadvantages:

- Any process or untrusted project code running as the guest user can silently
  become guest root.
- A compromised process can change the guest operating system and access
  credentials, files, and processes belonging to other users in the guest.
- It offers no guest privilege boundary to contain accidental destructive
  system changes.

## FAQ

> Why not running Codex directly on the host?

Auditing all commands that codex wants to run is not productive. Instead, by
running it in an isolated VM, you can run codex in yolo mode.

> Why not using one VM per project?

You will have a better isolation, but you will use more disk space and RAM.

Each VM has its own guest operating system, installed tools, package caches, and build artifacts, so this option uses more host disk space.

> Why lima instead of docker desktop?

Lima provides:

- A full Ubuntu VM rather than an Ubuntu container.
- A first-class SSH endpoint and generated OpenSSH configuration.
- It's an open source [CNCF project](https://www.cncf.io/projects/lima/), while Docker Desktop is a proprietary product.
- The Docker Desktop feature [Enhanced Container Isolation](https://docs.docker.com/enterprise/security/hardened-desktop/enhanced-container-isolation/), which "prevents malicious containers from compromising the host system" is restricted to Docker Business.

> I don't want to run codex with full privileges, I think it is dangerous!

Nobody forces you to run codex with full privileges. You can still run it in the
`buddy` VM for improved security _and_ customize its permissions.

## License

buddy is distributed under the terms of both the MIT license and the Apache
License (Version 2.0).

See [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT) for details.

## AI disclosure

We used AI to generate parts of this project, reviewing its output and making
modifications as needed.
