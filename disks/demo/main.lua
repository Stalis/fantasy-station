local x, y = 160, 120
local controller = input:get_controller()
local color = 0x7fff
local colors = {
    {"north", 0x7d4a},
    {"west", 0x2a7f},
    {"east", 0x7f6a},
    {"south", 0x2b6f},
}
local indicators = {"l1", "start", "r1", "l2", "select", "r2"}
local held = {}

function setup()
    graphics:fill(0x0843)
end

function update()
    if controller:down("left") then x = x - 2 end
    if controller:down("right") then x = x + 2 end
    if controller:down("up") then y = y - 2 end
    if controller:down("down") then y = y + 2 end
    x = math.max(12, math.min(graphics.width - 13, x))
    y = math.max(12, math.min(graphics.height - 13, y))

    for _, choice in ipairs(colors) do
        if controller:pressed(choice[1]) then
            color = choice[2]
        end
    end
    for i, name in ipairs(indicators) do
        held[i] = controller:down(name)
    end
end

function draw()
    graphics:fill(0x0843)
    graphics:circle(x, y, 12, color)

    for i in ipairs(indicators) do
        if held[i] then
            local cx = graphics.width - 40 + ((i - 1) % 3) * 14
            local cy = 10 + math.floor((i - 1) / 3) * 14
            graphics:circle(cx, cy, 4, 0x7fff)
        end
    end
end
