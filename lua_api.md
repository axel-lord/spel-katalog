# Lua API for batch scripts

## Types

## `Image`
Loaded image, has functions as defined bellow functions header.

## Functions

### `dbg(Any..) -> Any..`
Debug print and return all passed values.

### `loadYaml(path: String)`
Load yaml at path into a lua value.

### `loadCover(slug: String) -> Image | None`
Load a cover thumbnail from thumbnail cache.

### `loadImage(path: String) -> Image | None`
Load an image from given path.

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
values.

### `settings`
Current settings as a table.
