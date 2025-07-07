# spel-katalog
Swedish for game catalogue.

A viewer application for lutris libraries, with some additional features for
sandboxing games with firejail and different search/filtering options.

Inspired by microsofts old docx viewer application, and my personal game catalogue
growing a bit large.

# additional config
For some functionality the application might require extra configurations for
games, these are stored in a directory decided on by given settings, in 
*gameid*.toml files.

Additional data consists of custom attributes which the user may assign for scripts
and additional directories to allow when sandboxing, if no additional directory
is given the common parent of the prefix and executable will be used.

# scripts
Automation is cool, but somewhat outside of the scope of this project,
that said I added some capability to run scripts before a game is launched.

The intention was mostly to allow a wine prefix to be copied before first run,
or to remove any links to home created in the prefix, that said I made it
somewhat open for future extensions, and it may be used to deny running games
based on script decided conditions.

Script configs are toml or json files, placed in the folder specified by the
`--script-config-dir`/`script_config_dir` setting, they are ran sorted by file path.

For most of the string values in script configs string interpolation may be performed using `{VARIABLE}`,
`{` and `}` may be escaped using `{{` and `}}`.

The values which may not be interpolated are those under the `[global]` sectiona and key of environment
variables.

Available variables are as follows.
`HOME` for user home directory.

`ID` for game id.

`SLUG` for game slug.

`NANE` for game title.

`RUNNER` for game runner.

`HIDDEN` the string true if the game is hidden and false otherwise.

`EXE` path to the game executable.

`PREFIX` path to game wine prefix if using wine otherwise an empty string.

`ARCH` prefix architecture, if available else an empty string.

`GLOBAL.var` where var may be any string gets the value from `[global]` section of script config.

`ATTR.var` where var may be any string gets the value from additional game config
or if not available an empty string.
