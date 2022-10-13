#compdef sheldon

autoload -U is-at-least

_sheldon() {
    typeset -A opt_args
    typeset -a _arguments_options
    local ret=1

    if is-at-least 5.2; then
        _arguments_options=(-s -S -C)
    else
        _arguments_options=(-s -C)
    fi

    local context curcontext="$curcontext" state line
    _arguments "${_arguments_options[@]}" \
'--color=[Output coloring: always, auto, or never]:WHEN: ' \
'--config-dir=[The configuration directory]:PATH: ' \
'--data-dir=[The data directory]:PATH: ' \
'--config-file=[The config file]:PATH: ' \
'--profile=[The profile used for conditional plugins]:PROFILE: ' \
'-h[Print help information]' \
'--help[Print help information]' \
'-V[Print version information]' \
'--version[Print version information]' \
'-q[Suppress any informational output]' \
'--quiet[Suppress any informational output]' \
'-v[Use verbose output]' \
'--verbose[Use verbose output]' \
":: :_sheldon_commands" \
"*::: :->sheldon" \
&& ret=0
    case $state in
    (sheldon)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:sheldon-command-$line[1]:"
        case $line[1] in
            (init)
_arguments "${_arguments_options[@]}" \
'--shell=[The type of shell, accepted values are: bash, zsh]:SHELL: ' \
'-h[Print help information]' \
'--help[Print help information]' \
&& ret=0
;;
(add)
_arguments "${_arguments_options[@]}" \
'--git=[Add a clonable Git repository]:URL: ' \
'--gist=[Add a clonable Gist snippet]:ID: ' \
'--github=[Add a clonable GitHub repository]:REPO: ' \
'--remote=[Add a downloadable file]:URL: ' \
'--local=[Add a local directory]:DIR: ' \
'(--git --remote --local)--proto=[The Git protocol for a Gist or GitHub plugin]:PROTO: ' \
'--branch=[Checkout the tip of a branch]:BRANCH: ' \
'--rev=[Checkout a specific commit]:SHA: ' \
'--tag=[Checkout a specific tag]:TAG: ' \
'--dir=[Which sub directory to use in this plugin]:PATH: ' \
'*--use=[Which files to use in this plugin]:MATCH: ' \
'*--apply=[Templates to apply to this plugin]:TEMPLATE: ' \
'*--profiles=[Only use this plugin under one of the given profiles]:PROFILES: ' \
'-h[Print help information]' \
'--help[Print help information]' \
':name -- A unique name for this plugin:' \
&& ret=0
;;
(edit)
_arguments "${_arguments_options[@]}" \
'-h[Print help information]' \
'--help[Print help information]' \
&& ret=0
;;
(remove)
_arguments "${_arguments_options[@]}" \
'-h[Print help information]' \
'--help[Print help information]' \
':name -- A unique name for this plugin:' \
&& ret=0
;;
(lock)
_arguments "${_arguments_options[@]}" \
'--update[Update all plugin sources]' \
'(--update)--reinstall[Reinstall all plugin sources]' \
'-h[Print help information]' \
'--help[Print help information]' \
&& ret=0
;;
(source)
_arguments "${_arguments_options[@]}" \
'--relock[Regenerate the lock file]' \
'--update[Update all plugin sources (implies --relock)]' \
'(--update)--reinstall[Reinstall all plugin sources (implies --relock)]' \
'-h[Print help information]' \
'--help[Print help information]' \
&& ret=0
;;
(completions)
_arguments "${_arguments_options[@]}" \
'--shell=[The type of shell, accepted values are: bash, zsh]:SHELL: ' \
'-h[Print help information]' \
'--help[Print help information]' \
&& ret=0
;;
(version)
_arguments "${_arguments_options[@]}" \
'-h[Print help information]' \
'--help[Print help information]' \
&& ret=0
;;
        esac
    ;;
esac
}

(( $+functions[_sheldon_commands] )) ||
_sheldon_commands() {
    local commands; commands=(
'init:Initialize a new config file' \
'add:Add a new plugin to the config file' \
'edit:Open up the config file in the default editor' \
'remove:Remove a plugin from the config file' \
'lock:Install the plugins sources and generate the lock file' \
'source:Generate and print out the script' \
'completions:Generate completions for the given shell' \
'version:Prints detailed version information' \
    )
    _describe -t commands 'sheldon commands' commands "$@"
}
(( $+functions[_sheldon__add_commands] )) ||
_sheldon__add_commands() {
    local commands; commands=()
    _describe -t commands 'sheldon add commands' commands "$@"
}
(( $+functions[_sheldon__completions_commands] )) ||
_sheldon__completions_commands() {
    local commands; commands=()
    _describe -t commands 'sheldon completions commands' commands "$@"
}
(( $+functions[_sheldon__edit_commands] )) ||
_sheldon__edit_commands() {
    local commands; commands=()
    _describe -t commands 'sheldon edit commands' commands "$@"
}
(( $+functions[_sheldon__init_commands] )) ||
_sheldon__init_commands() {
    local commands; commands=()
    _describe -t commands 'sheldon init commands' commands "$@"
}
(( $+functions[_sheldon__lock_commands] )) ||
_sheldon__lock_commands() {
    local commands; commands=()
    _describe -t commands 'sheldon lock commands' commands "$@"
}
(( $+functions[_sheldon__remove_commands] )) ||
_sheldon__remove_commands() {
    local commands; commands=()
    _describe -t commands 'sheldon remove commands' commands "$@"
}
(( $+functions[_sheldon__source_commands] )) ||
_sheldon__source_commands() {
    local commands; commands=()
    _describe -t commands 'sheldon source commands' commands "$@"
}
(( $+functions[_sheldon__version_commands] )) ||
_sheldon__version_commands() {
    local commands; commands=()
    _describe -t commands 'sheldon version commands' commands "$@"
}

_sheldon "$@"
