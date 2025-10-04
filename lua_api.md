# Lua API for batch scripts

## Types

## `Image`
Loaded image, has functions as defined bellow functions header.

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

### `Image:w()`
Get image width.

### `Image:h()`
Get image height.

### `Image:save(path: String)`
Save image to given path.

### `Image:saveCover(slug: String)`
Save image as cover for given slug.

## Values

### `None`
A null value separate from lua nil, used for optional
values. Provided in the global scope.

### `settings`
Current settings as a table. Provided as a member of the module.

### `data`
Batch data, provided as a table/vector in global scope.
