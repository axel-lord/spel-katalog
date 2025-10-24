---@meta

---A color with a red, green, blue and alpha value.
---@class Color
---@field r number Red value of color between 0 an 255.
---@field g number Green value of color between 0 and 255.
---@field b number Blue value of color between 0 and 255.
---@field a number Alpha value of color between 0 and 1.
local Color = {}

---Add color metatable to given table, or create a new table with the metatable.
---@param tbl table? Table to add class to.
---@return Color
function Color:new(tbl) end

---Construct multiple new colors.
---@param ... table Tables to add class to.
---@return Color ...
function Color:new(...) end

---@class Rect
---@field x number X position of rectangle.
---@field y number Y position of rectangle.
---@field w number Width of rectangle.
---@field h number height of rectangle.
local Rect = {}

---Add rect metatable to given table, or create a new table with the metatable.
---@param tbl table? Table to add class to.
---@return Rect
function Rect:new(tbl) end

---Construct multiple new rects.
---@param ... table Tables to add class to.
---@return Rect ...
function Rect:new(...) end

---Letterboxing parameters.
---@alias Letterbox # How to letterbox an image.
---| nil # Use all defaults.
---| Color # Use provided color.
---| { ratio: number?, color: Color? } # Use provided values for ratio and color, or defaults.
---| number # Use provided ratio.

---Stretch/resize filter.
---@alias Filter # How to filter during resize.
---| "nearest" # Nearest filtering.
---| "triangle" # Triangle filtering.
---| "catmullrom" # Catmullrom filtering.
---| "gaussian" # Gaussian filtering.
---| "lanczos3" #  Lanczos3 filtering

---An in-memory image.
---@class Image
local Image = {}

---Load the cover of the given slug.
---@param slug string Game slug.
---@return (Image | [nil, string])
function Image:loadCover(slug) end

---Load the image at given path.
---@param path string Path to image.
---@return (Image | [nil, string])
function Image:load(path) end

---Create a new empty image.
---@param width number
---@param height number
---@return Image
function Image:new(width, height) end

---Get image width.
---@return number
function Image:w() end

---Get image height.
---@return number
function Image:h() end

---Get color at point.
---@param x number
---@param y number
---@return Color
function Image:at(x, y) end

---Set color at point.
---@param x number
---@param y number
---@param color Color
function Image:set(x, y, color) end

---Save image to path.
---@param path string Path to save image to.
function Image:save(path) end

---Save image as cover.
---@param slug string Slug to set image as cover for.
function Image:saveCover(slug) end

---Get average color.
---@return Color
function Image:avg() end

---Get letterboxed image.
---@param letterbox Letterbox
---@return Image
function Image:letterbox(letterbox) end

---Crop image to given rectangle.
---@param rect Rect
---@return Image
function Image:crop(rect) end

---Create an image flipped along the horizontal axis.
---@return Image
function Image:flipH() end

---Create an image flipped along the vertical axis.
---@return Image
function Image:flipV() end

---Overlay the other image onto self at the given position.
---@param other Image
---@param x number?
---@param y number?
---@return Image
function Image:overlay(other, x, y) end

---Return the image stretched to the given dimensions.
---@param w number
---@param h number
---@param filter Filter?
---@return Image
function Image:stretch(w, h, filter) end

---Return the image resized to at most the given dimensions with preserved aspect ratio.
---@param w number
---@param h number
---@param filter Filter?
---@return Image
function Image:resize(w, h, filter) end

---Map the colors of all pixels of the image using the provided function. If the function
---returns nil that pixel will be untouched.
---@param f fun(color: Color, x: number, y: number): Color?
---@return Image
function Image:mapColor(f) end

---A command to be ran.
---@class Command
local Command = {}


---Run the command returning the exit code if not interrupted.
---@return number?
function Command:status() end

---Create a new command with the current binary split ny shell splitting
---rules as new binary and initial arguments.
---@return Command
function Command:splitExec() end

---Run the command with the given optional input (given to command separated by newlines),
---returning a table with exit status, stderr and stdout.
---@param ... string Input to feed command stdin.
---@return { status: number?, stdout: string, stderr: string }
function Command:output(...) end

---A dialog box.
---@class Dialog
---@field buttons string[]
---@field text string
---@field ignore string[]
local Dialog = {}

---Add dialog metatable to given table, or create a new table with the metatable.
---@param tbl table? Table to add class to.
---@return Dialog
function Dialog:new(tbl) end

---Construct multiple new dialogs.
---@param ... table Tables to add class to.
---@return Dialog ...
function Dialog:new(...) end

---Open the dialog and wait for result, if closed nil is returned.
---@return string?
function Dialog:open() end

---A null value separate from lua nil, used for optional
---values. Provided in the module.
---@type lightuserdata
local None

---Game data given to batch and pre-launch scripts.
---@class GameData
---@field attrs {[string]: string} Custom attributes set for game.
---@field config string Path to game lutris yaml.
---@field hidden boolean Set to true if game is hidden.
---@field id number Lutris numeric id of game.
---@field name string Name of game.
---@field runner string Runner used by game.
---@field slug string Slug of game.
local GameData = {}

---Load lutris yml config for game.
---@return any
function GameData:loadConfig() end

---Load cached cover used by game.
---@return (Image | [nil, string])
function GameData:loadCover() end

---Save image to cover cache for this game.
---@param image Image
function GameData:saveCover(image) end

---Settings givent to batch and pre-launch scripts.
---@class Settings
---@field Theme string
---@field Show ("Apparent" | "Hidden" | "All")
---@field FilterMode ("Filter" | "Search" | "Regex")
---@field SortBy ("Id" | "Name" | "Slug")
---@field SortDir ("Forward" | "Reverse")
---@field Network ("Disabled" | "Enabled")
---@field LutrisExe string
---@field FirejailExe string
---@field CoverartDir string
---@field LutrisDb string
---@field YmlDir string
---@field ConfigDir string
---@field CacheDir string
local Settings = {}

---Current settings as a table. Provided as a member of the module.
---@type Settings
local settings

---Data for a single game, same format as values of batch data.
---Only available when running as a pre-launch script.
---@type GameData?
local game

---Batch data, provided as a table/vector in module.
---Only available when running batch scripts.
---@type GameData[]?
local data

local spelkatalog = {
	Color = Color,
	Rect = Rect,
	Image = Image,
	Dialog = Dialog,
	None = None,
	settings = settings,
	game = game,
	data = data,
}

---Debug print and return input values.
---@param ... any
---@return any ...
function spelkatalog.dbg(...) end

---Print all inputs using tostring.
---@param ... any
function spelkatalog.print(...) end

---Create a command.
---@param exec string Binary to execute.
---@param ... string Arguments to give binary.
---@return Command
function spelkatalog.cmd(exec, ...) end

---Read an environment variable.
---@param name string Name of variable.
---@return string?
function spelkatalog.getEnv(name) end

---Load yaml file at path.
---@param path string Path to file.
---@return any
function spelkatalog.loadYaml(path) end

---Load text file at path.
---@param path string Path to file.
---@return string
function spelkatalog.loadFile(path) end

---Save content to file at path.
---@param path string Path to file.
---@param content string Content to save.
function spelkatalog.saveFile(path, content) end

---Check if given path exists.
---@param path string Path to check
---@return boolean
function spelkatalog.pathExists(path) end

return spelkatalog
