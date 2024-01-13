# .zshrc generated by dawdle.space

autoload -U colors
colors
PROMPT="%{$fg[cyan]%}%n%{$reset_color%}@%{$fg[white]%}%M %(?:%{$fg_bold[green]%}➜ :%{$fg_bold[red]%}➜ ) %{$fg[cyan]%}%c%{$reset_color%} "

export ZSH="$HOME/.config/zsh"
export ZSH_CACHE_DIR="$ZSH/cache"
export ZSH_COMPDUMP="$ZSH_CACHE_DIR/zcompdump"
export ZSH_HISTFILE="$ZSH_CACHE_DIR/history"

export EDITOR='micro'
export PATH="$HOME/.local/bin:$PATH"
export LESSHISTFILE=-

alias ll='ls -la'
alias la='ls -la'
alias l='ls -l'
