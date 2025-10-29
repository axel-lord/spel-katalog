--Print the names of the first 3 games passed to script.

local sk = require'spel-katalog'

for i, game in ipairs(sk.data) do
	sk.dbg(game.name)
	if i >= 3 then
		break
	end
end
