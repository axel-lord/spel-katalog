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

Scripts may be written in lua, as of now the api is only available as a lua ls definition
file `lua/spel-katalog.lua`.
