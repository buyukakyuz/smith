# Agent Smith

Smith is a fast, open-source AI coding agent that runs in your terminal.

[![demo](https://asciinema.org/a/UxOkVwWsJqxYuABGEVWwOJVpJ.svg)](https://asciinema.org/a/UxOkVwWsJqxYuABGEVWwOJVpJ)

> **Warning**
> Under heavy development. Expect breaking changes.

## Getting Started

```bash
git clone https://github.com/buyukakyuz/smith.git
cd smith
cargo build --release
./target/release/smith
```

Run it from your project directory. Smith uses your current working directory as context.

Type `/model` to see available models and pick one.

## Configure

```bash
export ANTHROPIC_API_KEY=sk-...
export OPENAI_API_KEY=sk-...
```

## Commands

| Command | Action |
|---------|--------|
| `/model` | See available models and pick one |
| `/clear` | Clear conversation |
| `/help` | Show all commands |
| `/exit` | Quit |

## Tools

smith can read files, write files, search with glob and grep, list directories, and execute bash commands.

## License

[MIT](LICENSE)
