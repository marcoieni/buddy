# buddy

Run codex in a Linux VM using [lima](https://lima-vm.io/).
Control it from [ChatGPT desktop](https://chatgpt.com/download/).

The VM can have read-only access to Cloud services like DataDog and Fastly to debug issues.
Don't provide read access to any secrets or credentials.

Codex on the guest has only access to the `~/buddy` directory on the host.
So place there all the projects you want to work on with codex.

> [!NOTE]
> This project is meant for people working on the Rust Project infrastructure.
> It installs and configures tools `datadog`, and `fastly` that might not be
> needed for other projects.

## Set up

- Install the required tools on the host (eg on MacOS), along with a
  [Rust toolchain](https://rustup.rs/):

  ```sh
  brew install lima just
  ```

- Set up one password:
  1. Check if you have the 1Password CLI installed:

     ```sh
     op --version
     ```

  2. If not, install it with Homebrew:

     ```sh
     brew install 1password-cli
     ```

  3. If you have the 1password desktop app installed, enable:
     **Settings > Developer > Integrate with 1password CLI**.

- Create the VM:

  ```sh
  just create
  ```

- Make sure you can access the 1Password `Infrastructure` vault in the Rust-Foundation
  1Password. It contains the read-only credentials for Datadog and Fastly. If
  you are experiencing issues, check the
  [authentication docs](./docs/authentication.md).

- Login to Codex, Datadog and Fastly from the guest:

  ```sh
  just login
  ```

- Start the VM:

  ```sh
  just start
  ```

This is enough to run codex in the VM, but you can also connect from ChatGPT desktop
and use codex remotely.

### Connect from ChatGPT desktop

- Install [ChatGPT desktop](https://chatgpt.com/download/)
- After creating the VM, expose Lima's generated SSH configuration to OpenSSH:
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

- Configure the connection in ChatGPT desktop:

  1. Open **Settings > Connections > SSH**.
  2. Select **Add**, then select or enable `lima-buddy`.
  3. Choose `/work` as the remote project folder, or `/work/<project>` for a
     specific repository.
  4. Select `lima-buddy` as the run location when starting a task.

The host directory `~/buddy` is mounted inside the VM at `/work`, so
`~/buddy/<project>` on macOS is available as `/work/<project>` in ChatGPT.

See the [Lima SSH documentation](https://lima-vm.io/docs/usage/ssh/) and the
[ChatGPT remote connections documentation](https://learn.chatgpt.com/docs/remote-connections#connect-to-an-ssh-host)
for more details.

## Useful commands

- Check the [justfile](./justfile) for other available commands.

- Run commands in the VM:

  ```sh
  limactl shell buddy uname -m
  ```

## Security

- Read-only cloud tokens are still secrets and can be copied or abused. This
  is why we should use tokens with an expiration date.
  - Rotate tokens before they expire, and revoke a token immediately if the VM
    or token might have been compromised.
- Keep credentials out of ordinary cloud configuration. For example, Fastly `global:read`
  correctly treats VCL as readable configuration, so a credential embedded in
  VCL is exposed even though protected secret fields remain unavailable.

### Passwordless sudo

The template explicitly sets `user.passwordlessSudo: true`, which is
[Lima's default for Linux guests](https://lima-vm.io/docs/config/sudo/).
This gives Codex unrestricted root access inside the guest, but does not grant
root access on the host. The VM boundary and the resources exposed to the VM,
such as the writable `/work` mount, remain the main security boundary.

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

Disabling passwordless sudo has the inverse tradeoffs: it adds a deliberate
authentication step and limits an unprivileged compromise inside the guest, but
root operations become interactive and unattended tasks that need `sudo` may
block or fail. Lima generates an initial random password in `~/password`; rotate
it after the first login. System provisioning scripts declared with
`mode: system` still run as root.

There is also a current Lima limitation: on non-macOS guests,
`user.passwordlessSudo: false` requires `plain: true`. Plain mode disables the
guest agent and filesystem mounts, including Buddy's `/work` mount. Therefore,
disabling passwordless sudo is not currently a drop-in change for this
template; it also requires a different way to move project files into the VM,
such as SSH and `rsync`. The `user.passwordlessSudo` setting is experimental in
Lima 2.2 and this limitation may change.

## FAQ

> Why not running Codex directly on the host?

- Auditing every command Codex wants to run is not productive. The VM limits
  Codex to the guest and the resources explicitly exposed to it, so you can run
  Codex in yolo mode. By default, Codex still has root access inside the guest
  through [passwordless sudo](#passwordless-sudo).

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

You can still use a VM for host isolation and customize Codex's permissions.
See the [passwordless sudo tradeoffs](#passwordless-sudo) before removing guest
root access: Lima's current opt-out is not compatible with Buddy's `/work`
mount.
