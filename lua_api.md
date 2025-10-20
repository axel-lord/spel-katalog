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
Has three fields `status: Int | nil`, `stdout: String` and `Stderr: String`.

### `Color`
A table/class, is a result of `Image` functions.
Has four fields, integers betweeen 0 and 255, `r`, `g`, `b`, and a float between 0 and 1 `a`.

### `Filter`
A filter used for stretch and resize, may be one of nearest, triangle, catmullrom, gaussian and lanczos3.

### `Rect`
A table/class representing a rectangle.
Has four fields of type u32, `x`, `y`, `w`, `h`.
If the class constructor is used they default to 0, which may not have
a valid area for some image operations.

### `Dialog`
A table/class representing a dialog boc.
Has three fields, an array of strings `buttons` and text to display `text`, and optionally an array of
strings to ignore, that is return nil for `ignore`.

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

### `getEnv(name: String) -> String | nil`
Read an environment variable.

### `shellSplit(arg: String..) -> [String]`
Split the given input/s using shell splitting rules.

### `loadYaml(path: String) -> Value`
Load yaml at path into a lua value.

### `loadFile(path: String) -> String`
Load contents of a file to memory.

### `saveFile(path: String, content: String)`
Save content to given path.

### `pathExists(path: String) -> bool`
Check if the given path exists.

## Image
Functions provided for `Image`.

### `loadCover(class, slug: String) -> Image | nil, String`
Load a cover thumbnail from thumbnail cache.

### `load(class, path: String) -> Image | nil, String`
Load an image from given path.

### `new(class, width: Int, height: Int) -> Image`
Create a new image with the given width and height.

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

### `crop(self, Rect) -> Image`
Create a new cropped image.

### `flipH(self) -> Image`
Create an image flipped along the horizontal axis.

### `flipV(self) -> Image`
Create an image flipped along the vertical axis.

### `overlay(self, other: Image, x: u32 | nil, y: u32 | nil) -> Image`
Overlay the other image onto self at the given position.

### `stretch(self, w: u32, h: u32, filter: Filter | nil) -> Image`
Return the image stretched to the given dimensions.

### `resize(self, w: u32, h: u32, filter: Filter | nil) -> Image`
Return the image resized to at most the given dimensions with preserved aspect ratio.

### `mapColor(self, function(Color, x: u32, y: u32) -> Color | nil) -> Image`
Map the colors of all pixels of the image using the provided function. If the function
returns nil that pixel will be untouched.

## Color
Functions provided for `Color`.

### `new(class, initial: Table..) -> Color..`
Crate new colors either by adding the class to given tables, or
if no tables are provided by creating a new table with the class.

## Rect
Functions provided for `Rect`.

### `new(class, initial: Table..) -> Rect..`
Crate new rects either by adding the class to given tables, or
if no tables are provided by creating a new table with the class.

## Command
Functions provided for `Command`.

### `status(self) -> Int | nil`
Run the command returning the exit code if not interrupted.

### `splitExec(self) -> Command`
Create a new command with the current binary split ny shell splitting
rules as new binary and initial arguments.

### `output(self, input: String..) -> Output`
Run the command with the given optional input (given to command separated by newlines),
returning a table with exit status, stderr and stdout.

## Dialog
Functions provided for `Dialog`.

### `new(class, initial: Table..) -> Dialog..`
Crate new dialogs either by adding the class to given tables, or
if no tables are provided by creating a new table with the class.

### `open(self) -> String | nil`
Open the dialog and wait for result, if closed nil is returned.

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
