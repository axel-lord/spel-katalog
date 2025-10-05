# Lua API for batch scripts

## Conventions
This file is written following some conventions for describing the types
used by functions.

In these examples `Ty`, optionally followed by a number, may be any type.

### `Any`
May be any lua type.

### `Ty..`
A variadic input to a function with the given type.

### `[Ty]`
An array, a table of numbered entries from 1 upwards mapping to values
of `Ty`.

### `Ty1 | Ty2`
Either `Ty1` or `Ty2` 

## Types

### `Image`
Loaded image, has functions as defined bellow functions header.

### `Command`
An external command to be executed.

### `Output`
A table which is the result of `Command:output` being called.
Has three fields `status: Int | None`, `stdout: String` and `Stderr: String`.

## Functions
Functions are provided by the `"@spel-katalog"` module which has
to be required (`require'@spel-katalog'`).

### `dbg(Any..) -> Any..`
Debug print and return all passed values.

## `print(String)`
Print given string. Should be used for printing to make sure output is
captured correctly.

### `getEnv(name: String) -> String | None`
Read an environment variable.

### `shellSplit(arg: String..) -> [String]`
Split the given input/s using shell splitting rules.

### `loadYaml(path: String) -> Value`
Load yaml at path into a lua value.

### `loadFile(path: String) -> String`
Load contents of a file to memory.

### `loadCover(slug: String) -> Image | None`
Load a cover thumbnail from thumbnail cache.

### `loadImage(path: String) -> Image | None`
Load an image from given path.

### `saveFile(path: String, content: String)`
Save content to given path.

### `Image:w() -> Int`
Get image width.

### `Image:h() -> Int`
Get image height.

### `Image:save(path: String)`
Save image to given path.

### `Image:saveCover(slug: String)`
Save image as cover for given slug.

### `Command:status() -> Int | None`
Run the command returning the exit code if not interrupted.

### `Command:output(input: String..) -> Output`
Run the command with the given optional input (given to command separated by newlines),
returning a table with exit status, stderr and stdout.

## Values

### `None`
A null value separate from lua nil, used for optional
values. Provided in the global scope.

### `settings`
Current settings as a table. Provided as a member of the module.

### `data`
Batch data, provided as a table/vector in global scope.
