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

### `Color`
A table/class, is a result of `Image` functions.
Has four fields, integers betweeen 0 and 255, `r`, `g`, `b`, and a float between 0 and 1 `a`.

### `Letterbox`
The input to `Image:letterbox` may be one of several other types.
Ratio is expected to be width / height.
Default ratio is 1 and default color is full opacity black.
- `nil` Use default color and ratio.
- `Color` Use the provided color with default ratio.
- `Table { ratio: Int | Float | nil, color: Color | nil }` Use the provided ratio and color, or defaults.
- `Int | Float` Use the provided int or float as a ratio to letterbox by, default color is used.

## Functions
Functions are provided by the `"@spel-katalog"` module which has
to be required (`require'@spel-katalog'`).

### `dbg(Any..) -> Any..`
Debug print and return all passed values.

### `print(String)`
Print given string. Should be used for printing to make sure output is
captured correctly.

### `cmd(exec: String, arg: String..) -> Command`
Create a new command with the given executable and arguments.

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

### `newImage(width: Int, height: Int) -> Image`
Create a new image with the given width and height.

### `saveFile(path: String, content: String)`
Save content to given path.

### `pathExists(path: String) -> bool`
Check if the given path exists.

## Image
Functions provided for `Image`.

### `w(self) -> Int`
Get image width.

### `h(self) -> Int`
Get image height.

### `at(self, x: Int, y: Int) -> Color`
Get color at specified pixel.

### `set(self, x: Int, y: Int, Color)`
Set color at specified pixel.

### `save(self, path: String)`
Save image to given path.

### `saveCover(self, slug: String)`
Save image as cover for given slug.

### `avg(self) -> Color`
Get the average color of the image, with an alpha of 1.

### `letterbox(self, Letterbox) -> Image`
Create a new letterboxed image.

## Color
Functions provided for `Color`.

### `new(class, initial: Table...) -> Color...`
Crate new colors either by adding the class to given tables, or
if no tables are provided by creating a new table with the class.

## Command
Functions provided for `Command`.

### `status(self) -> Int | None`
Run the command returning the exit code if not interrupted.

### `splitExec(self) -> Command`
Create a new command with the current binary split ny shell splitting
rules as new binary and initial arguments.

### `output(self, input: String..) -> Output`
Run the command with the given optional input (given to command separated by newlines),
returning a table with exit status, stderr and stdout.

## Values

### `None`
A null value separate from lua nil, used for optional
values. Provided in the module.

### `settings`
Current settings as a table. Provided as a member of the module.

### `data`
Batch data, provided as a table/vector in module.
Only available when running batch scripts.

### `game`
Data for a single game, same format as values of batch data.
Only available when running as a pre-launch script.
