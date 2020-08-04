# Examples

This section demonstrates the configuration file contents for some popular
plugins and themes.

## Zsh frameworks

### [ohmyzsh](https://github.com/ohmyzsh/ohmyzsh)

Add the following to the **sheldon** config file.

```toml
[plugins.oh-my-zsh]
github = "ohmyzsh/ohmyzsh"
```

Or run the following to automatically add it.

```sh
sheldon add oh-my-zsh --github "ohmyzsh/ohmyzsh"
```

Add the following to your `~/.zshrc` file.

```sh
# ~/.zshrc

export ZSH="$HOME/.sheldon/repos/github.com/ohmyzsh/ohmyzsh"

# Oh My Zsh settings here

source <(sheldon source)
```

## Zsh plugins

### [autosuggestions](https://github.com/zsh-users/zsh-autosuggestions)

Add the following to the **sheldon** config file.

```toml
[plugins.zsh-autosuggestions]
github = "zsh-users/zsh-autosuggestions"
use = ["{{ name }}.zsh"]
```

Or run the following to automatically add it.

```sh
sheldon add zsh-autosuggestions --github zsh-users/zsh-autosuggestions --use '{{ name }}.zsh'
```

### [autojump](https://github.com/wting/autojump)

Add the following to the **sheldon** config file.

```toml
[plugins.autojump]
github = "wting/autojump"
dir = "bin"
apply = ["PATH", "source"]
```

Or run the following to automatically add it.

```sh
sheldon add autojump --github wting/autojump --dir bin --apply PATH source
```

### [syntax-highlighting](https://github.com/zsh-users/zsh-syntax-highlighting)

Add the following to the **sheldon** config file.

```toml
[plugins.zsh-syntax-highlighting]
github = "zsh-users/zsh-syntax-highlighting"
```

Or run the following to automatically add it.

```sh
sheldon add zsh-syntax-highlighting --github zsh-users/zsh-syntax-highlighting
```

### [blackbox](https://github.com/StackExchange/blackbox)

Add the following to the **sheldon** config file.

```toml
[plugins.blackbox]
github = "StackExchange/blackbox"
```

Or run the following to automatically add it.

```sh
sheldon add blackbox --github StackExchange/blackbox
```

### [z.lua](https://github.com/skywind3000/z.lua)

Add the following to the **sheldon** config file.

```toml
[plugins."z.lua"]
github = "skywind3000/z.lua"
```

Or run the following to automatically add it.

```sh
sheldon add z.lua --github skywind3000/z.lua
```

### [enhancd](https://github.com/b4b4r07/enhancd)

Add the following to the **sheldon** config file.

```toml
[plugins.enhancd]
github = "b4b4r07/enhancd"
```

Or run the following to automatically add it.

```sh
sheldon add enhancd --github b4b4r07/enhancd
```

### [base16](https://github.com/chriskempson/base16-shell)

Add the following to the **sheldon** config file.

```toml
[plugins.base16]
github = "chriskempson/base16-shell"
```

Or run the following to automatically add it.

```sh
sheldon add base16 --github chriskempson/base16-shell
```

## Zsh themes

### [powerlevel10k](https://github.com/romkatv/powerlevel10k)

Add the following to the **sheldon** config file.

```toml
[plugins.powerlevel10k]
github = "romkatv/powerlevel10k"
```

Or run the following to automatically add it.

```
sheldon add powerlevel10k --github romkatv/powerlevel10k
```

### [spaceship](https://github.com/denysdovhan/spaceship-prompt)

Add the following to the **sheldon** config file.

```toml
[plugins.spaceship]
github = "denysdovhan/spaceship-prompt"
```

Or run the following to automatically add it.

```sh
sheldon add spaceship --github denysdovhan/spaceship-prompt
```

### [pure](https://github.com/sindresorhus/pure)

Add the following to the **sheldon** config file.

```toml
[plugins.pure]
github = "sindresorhus/pure"
use = ["async.zsh", "pure.zsh"]
```

Or run the following to automatically add it.

```sh
sheldon add pure --github sindresorhus/pure --use async.zsh pure.zsh
```
