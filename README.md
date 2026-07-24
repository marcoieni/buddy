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

- Install the required tools on the host (eg on MacOS):

  ```sh
  brew install lima just
  ```
- Create the VM:

  ```sh
  just create
  ```
- Login to Codex, DataDog and Fastly from the guest

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

* Read-only cloud tokens are still secrets and can be copied or abused.
  It's better to login without tokens, e.g. using device auth, and to avoid copying any secrets into the VM.
  * It is recommended to periodically rotate any tokens that are used in the VM.

## FAQ

> Why not running Codex directly on the host?

* Auditing all commands that codex wants to run is not productive. Instead, by running in a VM without any privileges, you can run codex in yolo mode.

> Why not using one VM per project?

You will have a better isolation, but you will use more disk space and RAM.

Each VM has its own guest operating system, installed tools, package caches, and build artifacts, so this option uses more host disk space.

> Why lima instead of docker desktop?

Lima provides:
* A full Ubuntu VM rather than an Ubuntu container.
* A first-class SSH endpoint and generated OpenSSH configuration.
* It's an open source [CNCF project](https://www.cncf.io/projects/lima/), while Docker Desktop is a proprietary product.
* The Docker Desktop feature [Enhanced Container Isolation](https://docs.docker.com/enterprise/security/hardened-desktop/enhanced-container-isolation/), which "prevents malicious containers from compromising the host system" is restricted to Docker Business.
